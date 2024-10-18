//! # DID Key Operations
//!
//! Implements Create, Read, Update, Delete (CRUD) operations for DID Key.
//!
//! See <https://w3c-ccg.github.io/did-method-key>

use anyhow::anyhow;
use base64ct::{Base64UrlUnpadded, Encoding};
use curve25519_dalek::edwards::CompressedEdwardsY;
use serde_json::json;

use super::DidJwk;
use crate::core::Kind;
use crate::document::{
    CreateOptions, Document, MethodType, PublicKey, PublicKeyFormat, VerificationMethod,
};
use crate::error::Error;
use crate::{DidOperator, KeyPurpose};

impl DidJwk {
    pub fn create(op: impl DidOperator, options: CreateOptions) -> crate::Result<Document> {
        let Some(verifying_key) = op.verification(KeyPurpose::VerificationMethod) else {
            return Err(Error::Other(anyhow!("no verification key")));
        };

        let serialized = serde_json::to_vec(&verifying_key)
            .map_err(|e| Error::Other(anyhow!("issue serializing key: {e}")))?;
        let encoded = Base64UrlUnpadded::encode_string(&serialized);
        let did = format!("did:jwk:{encoded}");

        // key agreement
        // <https://w3c-ccg.github.io/did-method-key/#encryption-method-creation-algorithm>
        let key_agreement = if options.enable_encryption_key_derivation {
            let key_bytes = Base64UrlUnpadded::decode_vec(&verifying_key.x)
                .map_err(|e| Error::InvalidPublicKey(format!("issue decoding key: {e}")))?;

            // derive an X25519 public encryption key from the Ed25519 key
            let edwards_y = CompressedEdwardsY::from_slice(&key_bytes).map_err(|e| {
                Error::InvalidPublicKey(format!("public key is not Edwards Y: {e}"))
            })?;
            let Some(edwards_pt) = edwards_y.decompress() else {
                return Err(Error::InvalidPublicKey(
                    "Edwards Y cannot be decompressed to point".into(),
                ));
            };
            let x25519_bytes = edwards_pt.to_montgomery().to_bytes();

            let mut jwk = verifying_key.clone();
            jwk.x = Base64UrlUnpadded::encode_string(&x25519_bytes);
            let method_type = MethodType::JsonWebKey { public_key_jwk: jwk };

            Some(vec![Kind::Object(VerificationMethod {
                id: format!("{did}#key-1"),
                controller: did.clone(),
                method_type,
                ..VerificationMethod::default()
            })])
        } else {
            None
        };

        let verif_type = &options.public_key_format;
        let (context, public_key) = (
            Kind::Object(json!({
                "publicKeyJwk": {
                    "@id": "https://w3id.org/security#publicKeyJwk",
                    "@type": "@json"
                },
                verif_type.to_string(): format!("https://w3id.org/security#{verif_type}"),
            })),
            PublicKey::Jwk(verifying_key),
        );

        let kid = format!("{did}#key-0");

        let method_type = match options.public_key_format {
            PublicKeyFormat::Multikey => MethodType::Multikey {
                public_key_multibase: public_key.multibase().unwrap(),
            },
            _ => MethodType::JsonWebKey {
                public_key_jwk: public_key.jwk().unwrap(),
            },
        };

        Ok(Document {
            context: vec![Kind::String(options.default_context), context],
            id: did.clone(),
            verification_method: Some(vec![VerificationMethod {
                id: kid.clone(),
                controller: did,
                method_type,
                ..VerificationMethod::default()
            }]),
            authentication: Some(vec![Kind::String(kid.clone())]),
            assertion_method: Some(vec![Kind::String(kid.clone())]),
            capability_invocation: Some(vec![Kind::String(kid.clone())]),
            capability_delegation: Some(vec![Kind::String(kid)]),
            key_agreement,
            ..Document::default()
        })
    }

    #[allow(dead_code)]
    pub fn read(_did: &str, _: CreateOptions) -> crate::Result<Document> {
        // self.resolve(did, options)
        unimplemented!("read")
    }
}

#[cfg(test)]
mod test {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use vercre_infosec::{Curve, KeyType, PublicKeyJwk};

    use super::*;

    #[test]
    fn create() {
        let mut options = CreateOptions::default();
        options.enable_encryption_key_derivation = true;

        let op = Operator;
        let res = DidJwk::create(op, options).expect("should create");

        let json = serde_json::to_string_pretty(&res).expect("should serialize");
        println!("{json}");
    }

    struct Operator;
    impl DidOperator for Operator {
        fn verification(&self, purpose: KeyPurpose) -> Option<PublicKeyJwk> {
            match purpose {
                KeyPurpose::VerificationMethod => {
                    let key = generate();

                    Some(PublicKeyJwk {
                        kty: KeyType::Okp,
                        crv: Curve::Ed25519,
                        x: Base64UrlUnpadded::encode_string(&key),
                        ..PublicKeyJwk::default()
                    })
                }
                _ => panic!("unsupported purpose"),
            }
        }
    }

    // HACK: generate a key pair
    #[allow(dead_code)]
    pub fn generate() -> Vec<u8> {
        // TODO: pass in public key
        let mut csprng = OsRng;
        let signing_key: SigningKey = SigningKey::generate(&mut csprng);
        let secret = Base64UrlUnpadded::encode_string(signing_key.as_bytes());
        println!("signing: {secret}");

        signing_key.verifying_key().to_bytes().to_vec()
    }
}