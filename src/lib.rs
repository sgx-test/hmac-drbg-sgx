#![no_std]

#![cfg_attr(all(feature = "mesalock_sgx", not(target_env = "sgx")), no_std)]
#![cfg_attr(all(target_env = "sgx", target_vendor = "mesalock"), feature(rustc_private))]

#[cfg(all(feature = "mesalock_sgx", not(target_env = "sgx")))]
#[macro_use]
extern crate sgx_tstd as std;

use std::prelude::v1::*;
use digest::{Input, BlockInput, FixedOutput, Reset};
use generic_array::{ArrayLength, GenericArray};
use hmac::{Mac, Hmac};

pub struct HmacDRBG<D>
    where D: Input + BlockInput + FixedOutput + Default,
          D::BlockSize: ArrayLength<u8>,
          D::OutputSize: ArrayLength<u8>
{
    k: GenericArray<u8, D::OutputSize>,
    v: GenericArray<u8, D::OutputSize>,
    count: usize,
}

impl<D> HmacDRBG<D> where
    D: Input + FixedOutput + BlockInput + Reset + Clone + Default,
    D::BlockSize: ArrayLength<u8>,
    D::OutputSize: ArrayLength<u8>
{
    pub fn new(entropy: &[u8], nonce: &[u8], pers: &[u8]) -> Self {
        let mut k = GenericArray::<u8, D::OutputSize>::default();
        let mut v = GenericArray::<u8, D::OutputSize>::default();

        for i in 0..k.as_slice().len() {
            k[i] = 0x0;
        }

        for i in 0..v.as_slice().len() {
            v[i] = 0x01;
        }

        let mut this = Self {
            k, v,
            count: 0,
        };

        this.update(Some(&[entropy, nonce, pers]));
        this.count = 1;

        this
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn reseed(&mut self, entropy: &[u8], add: Option<&[u8]>) {
        self.update(Some(&[entropy, add.unwrap_or(&[])]))
    }

    pub fn generate<T: ArrayLength<u8>>(&mut self, add: Option<&[u8]>) -> GenericArray<u8, T> {
        let mut result = GenericArray::default();
        self.generate_to_slice(result.as_mut_slice(), add);
        result
    }

    pub fn generate_to_slice(&mut self, result: &mut [u8], add: Option<&[u8]>) {
        if let Some(add) = add {
            self.update(Some(&[add]));
        }

        let mut i = 0;
        while i < result.len() {
            let mut vmac = self.hmac();
            vmac.input(&self.v);
            self.v = vmac.result().code();

            for j in 0..self.v.len() {
                result[i + j] = self.v[j];
            }
            i += self.v.len();
        }

        match add {
            Some(add) => {
                self.update(Some(&[add]));
            },
            None => {
                self.update(None);
            },
        }
        self.count += 1;
    }

    fn hmac(&self) -> Hmac<D> {
        Hmac::new_varkey(&self.k).expect("Smaller and larger key size are handled by default")
    }

    fn update(&mut self, seeds: Option<&[&[u8]]>) {
        let mut kmac = self.hmac();
        kmac.input(&self.v);
        kmac.input(&[0x00]);
        if let Some(seeds) = seeds {
            for seed in seeds {
                kmac.input(seed);
            }
        }
        self.k = kmac.result().code();

        let mut vmac = self.hmac();
        vmac.input(&self.v);
        self.v = vmac.result().code();

        if seeds.is_none() {
            return;
        }

        let seeds = seeds.unwrap();

        let mut kmac = self.hmac();
        kmac.input(&self.v);
        kmac.input(&[0x01]);
        for seed in seeds {
            kmac.input(seed);
        }
        self.k = kmac.result().code();

        let mut vmac = self.hmac();
        vmac.input(&self.v);
        self.v = vmac.result().code();
    }
}
