//! Elliptic Curve Digital Signature Algorithm (ECDSA) as specified in
//! [FIPS 186-4] (Digital Signature Standard)
//!
//! ## About
//!
//! This crate provides generic ECDSA support which can be used in the
//! following ways:
//!
//! - Generic implementation of ECDSA usable with the following crates:
//!   - [`k256`] (secp256k1)
//!   - [`p256`] (NIST P-256)
//! - ECDSA signature types alone which can be used to provide interoperability
//!   between other crates that provide an ECDSA implementation:
//!   - [`p384`] (NIST P-384)
//!
//! Any crates which provide an implementation of ECDSA for a particular
//! elliptic curve can leverage the types from this crate, along with the
//! [`k256`], [`p256`], and/or [`p384`] crates to expose ECDSA functionality in
//! a generic, interoperable way by leveraging the [`Signature`] type with in
//! conjunction with the [`signature_flow::Signer`] and [`signature_flow::Verifier`]
//! traits.
//!
//! For example, the [`ring-compat`] crate implements the [`signature_flow::Signer`]
//! and [`signature_flow::Verifier`] traits in conjunction with the
//! [`p256::ecdsa::Signature`] and [`p384::ecdsa::Signature`] types to
//! wrap the ECDSA implementations from [*ring*] in a generic, interoperable
//! API.
//!
//! ## Minimum Supported Rust Version
//!
//! Rust **1.52** or higher.
//!
//! Minimum supported Rust version may be changed in the future, but it will be
//! accompanied with a minor version bump.
//!
//! [FIPS 186-4]: https://csrc.nist.gov/publications/detail/fips/186/4/final
//! [`k256`]: https://docs.rs/k256
//! [`p256`]: https://docs.rs/p256
//! [`p256::ecdsa::Signature`]: https://docs.rs/p256/latest/p256/ecdsa/type.Signature.html
//! [`p384`]: https://docs.rs/p384
//! [`p384::ecdsa::Signature`]: https://docs.rs/p384/latest/p384/ecdsa/type.Signature.html
//! [`ring-compat`]: https://docs.rs/ring-compat
//! [*ring*]: https://docs.rs/ring

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code, clippy::unwrap_used)]
#![warn(missing_docs, rust_2018_idioms)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/8f1a9894/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/8f1a9894/logo.svg",
    html_root_url = "https://docs.rs/ecdsa/0.13.0-pre"
)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "der")]
#[cfg_attr(docsrs, doc(cfg(feature = "der")))]
pub mod der;

#[cfg(feature = "dev")]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
pub mod dev;

#[cfg(feature = "hazmat")]
#[cfg_attr(docsrs, doc(cfg(feature = "hazmat")))]
pub mod hazmat;

#[cfg(feature = "sign")]
#[cfg_attr(docsrs, doc(cfg(feature = "sign")))]
pub mod rfc6979;

#[cfg(feature = "sign")]
mod sign;

#[cfg(feature = "verify")]
mod verify;

// Re-export the `elliptic-curve` crate (and select types)
pub use elliptic_curve_flow::{self, sec1::EncodedPoint, PrimeCurve};

// Re-export the `signature` crate (and select types)
pub use signature_flow::{self, Error, Result};

#[cfg(feature = "sign")]
#[cfg_attr(docsrs, doc(cfg(feature = "sign")))]
pub use sign::SigningKey;

#[cfg(feature = "verify")]
#[cfg_attr(docsrs, doc(cfg(feature = "verify")))]
pub use verify::VerifyingKey;

use core::{
    convert::TryFrom,
    fmt::{self, Debug},
    ops::Add,
};
use elliptic_curve_flow::{
    bigint::Encoding as _,
    generic_array::{sequence::Concat, ArrayLength, GenericArray},
    FieldBytes, FieldSize, ScalarCore,
};

#[cfg(feature = "arithmetic")]
use elliptic_curve_flow::{group::ff::PrimeField, NonZeroScalar, ProjectiveArithmetic, Scalar};

/// Size of a fixed sized signature for the given elliptic curve.
pub type SignatureSize<C> = <FieldSize<C> as Add>::Output;

/// Fixed-size byte array containing an ECDSA signature
pub type SignatureBytes<C> = GenericArray<u8, SignatureSize<C>>;

/// ECDSA signature (fixed-size). Generic over elliptic curve types.
///
/// Serialized as fixed-sized big endian scalar values with no added framing:
///
/// - `r`: field element size for the given curve, big-endian
/// - `s`: field element size for the given curve, big-endian
///
/// For example, in a curve with a 256-bit modulus like NIST P-256 or
/// secp256k1, `r` and `s` will both be 32-bytes, resulting in a signature
/// with a total of 64-bytes.
///
/// ASN.1 DER-encoded signatures also supported via the
/// [`Signature::from_der`] and [`Signature::to_der`] methods.
#[derive(Clone, Eq, PartialEq)]
pub struct Signature<C: PrimeCurve>
where
    SignatureSize<C>: ArrayLength<u8>,
{
    bytes: SignatureBytes<C>,
}

impl<C> Signature<C>
where
    C: PrimeCurve,
    SignatureSize<C>: ArrayLength<u8>,
{
    /// Parse a signature from ASN.1 DER
    #[cfg(feature = "der")]
    #[cfg_attr(docsrs, doc(cfg(feature = "der")))]
    pub fn from_der(bytes: &[u8]) -> Result<Self>
    where
        der::MaxSize<C>: ArrayLength<u8>,
        <FieldSize<C> as Add>::Output: Add<der::MaxOverhead> + ArrayLength<u8>,
    {
        der::Signature::<C>::try_from(bytes).and_then(Self::try_from)
    }

    /// Create a [`Signature`] from the serialized `r` and `s` scalar values
    /// which comprise the signature.
    pub fn from_scalars(r: impl Into<FieldBytes<C>>, s: impl Into<FieldBytes<C>>) -> Result<Self> {
        Self::try_from(r.into().concat(s.into()).as_slice())
    }

    /// Split the signature into its `r` and `s` components, represented as bytes.
    pub fn split_bytes(&self) -> (FieldBytes<C>, FieldBytes<C>) {
        let (r_bytes, s_bytes) = self.bytes.split_at(C::UInt::BYTE_SIZE);

        (
            GenericArray::clone_from_slice(r_bytes),
            GenericArray::clone_from_slice(s_bytes),
        )
    }

    /// Serialize this signature as ASN.1 DER
    #[cfg(feature = "der")]
    #[cfg_attr(docsrs, doc(cfg(feature = "der")))]
    pub fn to_der(&self) -> der::Signature<C>
    where
        der::MaxSize<C>: ArrayLength<u8>,
        <FieldSize<C> as Add>::Output: Add<der::MaxOverhead> + ArrayLength<u8>,
    {
        let (r, s) = self.bytes.split_at(C::UInt::BYTE_SIZE);
        der::Signature::from_scalar_bytes(r, s).expect("DER encoding error")
    }
}

#[cfg(feature = "arithmetic")]
#[cfg_attr(docsrs, doc(cfg(feature = "arithmetic")))]
impl<C> Signature<C>
where
    C: PrimeCurve + ProjectiveArithmetic,
    SignatureSize<C>: ArrayLength<u8>,
{
    /// Get the `r` component of this signature
    pub fn r(&self) -> NonZeroScalar<C> {
        NonZeroScalar::try_from(self.split_bytes().0.as_slice())
            .expect("r-component ensured valid in constructor")
    }

    /// Get the `s` component of this signature
    pub fn s(&self) -> NonZeroScalar<C> {
        NonZeroScalar::try_from(self.split_bytes().1.as_slice())
            .expect("s-component ensured valid in constructor")
    }

    /// Split the signature into its `r` and `s` scalars.
    pub fn split_scalars(&self) -> (NonZeroScalar<C>, NonZeroScalar<C>) {
        (self.r(), self.s())
    }

    /// Normalize signature into "low S" form as described in
    /// [BIP 0062: Dealing with Malleability][1].
    ///
    /// [1]: https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki
    pub fn normalize_s(&self) -> Option<Self>
    where
        Scalar<C>: NormalizeLow,
    {
        self.s().normalize_low().map(|s_low| {
            let mut result = self.clone();
            result.bytes[C::UInt::BYTE_SIZE..].copy_from_slice(&s_low.to_repr());
            result
        })
    }
}

impl<C> signature_flow::Signature for Signature<C>
where
    C: PrimeCurve,
    SignatureSize<C>: ArrayLength<u8>,
{
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::try_from(bytes)
    }
}

impl<C> AsRef<[u8]> for Signature<C>
where
    C: PrimeCurve,
    SignatureSize<C>: ArrayLength<u8>,
{
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

impl<C> Copy for Signature<C>
where
    C: PrimeCurve,
    SignatureSize<C>: ArrayLength<u8>,
    <SignatureSize<C> as ArrayLength<u8>>::ArrayType: Copy,
{
}

impl<C> Debug for Signature<C>
where
    C: PrimeCurve,
    SignatureSize<C>: ArrayLength<u8>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ecdsa::Signature<{:?}>({:?})",
            C::default(),
            self.as_ref()
        )
    }
}

impl<C> TryFrom<&[u8]> for Signature<C>
where
    C: PrimeCurve,
    SignatureSize<C>: ArrayLength<u8>,
{
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != C::UInt::BYTE_SIZE * 2 {
            return Err(Error::new());
        }

        for scalar_bytes in bytes.chunks_exact(C::UInt::BYTE_SIZE) {
            let scalar = ScalarCore::<C>::from_be_slice(scalar_bytes).map_err(|_| Error::new())?;

            if scalar.is_zero().into() {
                return Err(Error::new());
            }
        }

        Ok(Self {
            bytes: GenericArray::clone_from_slice(bytes),
        })
    }
}

/// Normalize a scalar (i.e. ECDSA S) to the lower half the field, as described
/// in [BIP 0062: Dealing with Malleability][1].
///
/// [1]: https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki
pub trait NormalizeLow: Sized {
    /// Normalize scalar to the lower half of the field (i.e. negate it if it's
    /// larger than half the curve's order).
    ///
    /// Returns an `Option` with a new scalar if the original wasn't already
    /// low-normalized.
    ///
    /// May be implemented to work in variable time.
    fn normalize_low(&self) -> Option<Self>;
}
