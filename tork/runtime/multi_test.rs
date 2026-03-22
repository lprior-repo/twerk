//! Tests for the runtime module

#[cfg(test)]
mod tests {
    use crate::runtime::{Mounter, MultiMounter};
    use crate::mount::Mount;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    // A fake mounter for testing
    struct FakeMounter {
        mount_type: String,
    }

    impl Mounter for FakeMounter {
        fn mount(
            &self,
            _ctx: Pin<Box<dyn Future<Output = Result<(), crate::runtime::mount::MountError>> + Send>>,
            mnt: &Mount,
        ) -> Pin<Box<dyn Future<Output = Result<(), crate::runtime::mount::MountError>> + Send>> {
            let mut mnt = mnt.clone();
            mnt.mount_type = self.mount_type.clone();
            Box::pin(async move {
                // Simulate mount operation
                Ok(())
            })
        }

        fn unmount(
            &self,
            _ctx: Pin<Box<dyn Future<Output = Result<(), crate::runtime::mount::MountError>> + Send>>,
            _mnt: &Mount,
        ) -> Pin<Box<dyn Future<Output = Result<(), crate::runtime::mount::MountError>> + Send>> {
            Box::pin(async move {
                // Simulate unmount operation
                Ok(())
            })
        }
    }

    impl Clone for FakeMounter {
        fn clone(&self) -> Self {
            Self {
                mount_type: self.mount_type.clone(),
            }
        }
    }

    #[tokio::test]
    async fn test_multi_mounter_register() {
        let mut mounter = MultiMounter::new();
        let fake = FakeMounter {
            mount_type: "volume".to_string(),
        };
        mounter
            .register_mounter("volume", Box::new(fake))
            .await;
    }

    #[tokio::test]
    async fn test_multi_mounter_mount_unknown_type() {
        let mounter = MultiMounter::new();
        let mount = Mount {
            id: Some("mount-1".to_string()),
            mount_type: "unknown".to_string(),
            target: Some("/mnt".to_string()),
            ..Mount::default()
        };

        let result = mounter.mount(&mount).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multi_mounter_mount_missing_id() {
        let mounter = MultiMounter::new();
        let mount = Mount {
            id: None,
            mount_type: "volume".to_string(),
            target: Some("/mnt".to_string()),
            ..Mount::default()
        };

        let result = mounter.mount(&mount).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multi_mounter_unmount_missing_id() {
        let mounter = MultiMounter::new();
        let mount = Mount {
            id: None,
            mount_type: "volume".to_string(),
            target: Some("/mnt".to_string()),
            ..Mount::default()
        };

        let result = mounter.unmount(&mount).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multi_mounter_unmount_not_found() {
        let mounter = MultiMounter::new();
        let mount = Mount {
            id: Some("nonexistent".to_string()),
            mount_type: "volume".to_string(),
            target: Some("/mnt".to_string()),
            ..Mount::default()
        };

        let result = mounter.unmount(&mount).await;
        assert!(result.is_err());
    }
}
