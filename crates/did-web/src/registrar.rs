use did_core::error::Err;
use did_core::hashing::rand_hex;
use did_core::{
    tracerr, Action, Context, DidDocument, KeyOperation, KeyPurpose, KeyRing, Patch, Registrar,
    Result, Service, Signer, VerificationMethod, VmWithPurpose, DID_CONTEXT,
};

use crate::Registrar as WebRegistrar;

/// DID Registrar implementation for the Web method.
impl<K> Registrar for WebRegistrar<K>
where
    K: KeyRing + Signer + Send + Sync,
{
    /// There is intentionally no HTTP API specified for did:web method operations leaving
    /// programmatic registrations and management to be defined by each implementation, or based on
    /// their own requirements in their web environment.
    ///
    /// This function will construct a DID document for the specified services and create a
    /// verification method for use in authentication and assertion, thus being useful for
    /// verifiable credential issuance.
    ///
    /// The returned document will have no ID, so it is up to the caller to assign one and host it.
    async fn create(&self, services: Option<&[Service]>) -> Result<DidDocument> {
        let signing_key = self.keyring.next_key(&KeyOperation::Sign).await?;
        let algorithm = match signing_key.check(&self.keyring.supported_algorithms()) {
            Ok(a) => a,
            Err(e) => tracerr!(e, "Signing key error"),
        };

        let mut doc = DidDocument {
            context: vec![Context {
                url: Some(DID_CONTEXT.to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let vm = VmWithPurpose {
            verification_method: VerificationMethod {
                id: rand_hex(8),
                controller: self.controller.clone().unwrap_or_default(),
                type_: algorithm.cryptosuite(),
                public_key_jwk: Some(signing_key.clone()),
                ..Default::default()
            },
            purposes: Some(vec![KeyPurpose::Authentication, KeyPurpose::AssertionMethod]),
        };
        let patch_key = Patch::builder(Action::AddPublicKeys).public_key(&vm)?.build()?;
        doc.apply_patches(&[patch_key]);

        if let Some(svcs) = services {
            let mut patch_service_builder = Patch::builder(Action::AddServices);
            for s in svcs {
                patch_service_builder.service(s)?;
            }
            let patch_service = patch_service_builder.build()?;
            doc.apply_patches(&[patch_service]);
        }

        Ok(doc)
    }

    /// Construct a new DID document by applying patches to an existing document.
    async fn update(&self, doc: &DidDocument, patches: &[Patch]) -> Result<DidDocument> {
        let mut new_doc = doc.clone();
        new_doc.apply_patches(patches);
        Ok(new_doc)
    }

    /// This function is not supported for the Web method. Deactivation is done by removing the
    /// document from the hosting environment.
    async fn deactivate(&self, _: &str) -> Result<()> {
        tracerr!(Err::NotSupported)
    }

    /// This function is not supported for the Web method. Recovery is done by re-hosting a document
    /// that had previously been removed..
    async fn recover(&self, _: &DidDocument) -> Result<()> {
        tracerr!(Err::NotSupported)
    }

    /// Declare the DID method for this registrar.
    fn method() -> String {
        "web".to_owned()
    }
}