extern crate rand;
extern crate serialize;

use bignum::BigNumTrait;
use bignum::NumBigInt as BigNum;
use mac::hmac_sha256;
use sha2::{Sha256, Digest};

use rand::Rng;

pub enum LoginResult {
    Success,
    Failure
}

#[derive(Debug)]
pub struct SRP {
    N: BigNum,
    g: BigNum,
    k: BigNum,
}

pub fn serialize<T: BigNumTrait>(x: &T) -> Vec<u8> {
    x.to_bytes_be()
}

pub fn deserialize<T: BigNumTrait>(x: &[u8]) -> T {
    T::from_bytes_be(x)
}

impl SRP {
    pub fn new() -> Self {
        let N_hex = "ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74\
                     020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f1437\
                     4fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7ed\
                     ee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf05\
                     98da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb\
                     9ed529077096966d670c354e4abc9804f1746c08ca237327ffffffffffffffff";

        let N = BigNum::from_hex_str(N_hex).unwrap();
        let g = BigNum::from_u32(2);
        let k = BigNum::from_u32(3);
        SRP {
            N,
            g,
            k,
        }
    }

    pub fn password_to_secret(&self, password: &[u8]) -> (Vec<u8>, BigNum) {
        let mut rng = rand::thread_rng();
        // Which size should the salt have?
        let salt: Vec<u8> = rng.gen_iter::<u8>().take(128).collect();

        let x = compute_x(&salt, password);
        (salt, self.g.mod_exp(&x, &self.N))
    }
}

struct HandshakeState<'a> {
    srp: &'a SRP,
    exponent: BigNum,
    power: BigNum,
}

impl<'a> HandshakeState<'a> {
    pub fn new(srp: &'a SRP) -> Self {
        let exponent = BigNum::gen_below(&srp.N);
        let power = srp.g.mod_exp(&exponent, &srp.N);
        HandshakeState {
            srp,
            exponent,
            power,
        }
    }
}

pub struct ClientHandshake<'a> {
    state: HandshakeState<'a>,
    password: &'a [u8],
}

impl <'a> ClientHandshake<'a> {
    pub fn new(srp: &'a SRP, password: &'a [u8]) -> Self {
        ClientHandshake {
            state: HandshakeState::new(srp),
            password
        }
    }

    pub fn A(&self) -> &BigNum {
        &self.state.power
    }

    pub fn compute_secret(&self, B: &BigNum, salt: &[u8]) -> Vec<u8> {
        let state = &self.state;
        let srp = state.srp;
        let N = &srp.N;
        let g = &srp.g;
        let k = &srp.k;
        let a = &state.exponent;
        let A = &state.power;

        let u = compute_u(A, B);
        let x = compute_x(salt, self.password);

        let S = (B - &(k * &g.mod_exp(&x, N))).mod_exp(&(a + &(&u * &x)), N);
        let K = Sha256::digest(&serialize(&S)).to_vec();
        hmac_sha256(&K, salt)
    }
}

pub struct ServerHandshake<'a> {
    state: HandshakeState<'a>,
    B: BigNum,
    salt: &'a [u8],
    v: &'a BigNum,
}

impl <'a> ServerHandshake<'a> {
    pub fn new(srp: &'a SRP, salt: &'a [u8], v: &'a BigNum) -> Self {
        let state = HandshakeState::new(srp);
        let B = &state.power + &(&srp.k * v);
        ServerHandshake {
            state,
            B,
            salt,
            v,
        }
    }

    pub fn B(&self) -> &BigNum {
        &self.B
    }

    pub fn compute_secret(&self, A: &BigNum) -> Vec<u8> {
        let state = &self.state;
        let srp = state.srp;
        let N = &srp.N;
        let b = &state.exponent;
        let B = &self.B;

        let u = compute_u(A, B);
        let S = (A * &self.v.mod_exp(&u, N)).mod_exp(b, N);
        let K = Sha256::digest(&serialize(&S)).to_vec();
        hmac_sha256(&K, self.salt)
    }
}

fn compute_u(A: &BigNum, B: &BigNum) -> BigNum {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&serialize(A));
    buffer.extend_from_slice(&serialize(B));
    deserialize(&Sha256::digest(&buffer))
}

fn compute_x(salt: &[u8], password: &[u8]) -> BigNum {
    let mut buffer = Vec::with_capacity(salt.len() + password.len());
    buffer.extend_from_slice(salt);
    buffer.extend_from_slice(password);
    deserialize(&Sha256::digest(&buffer))
}
