//! Image management logic for PodmanRuntime.

use std::sync::Arc;
6: use std::time::{Duration, Instant};

 8: 
 9: use tokio::io::AsyncWriteExt;
10: use tokio::process::Command;
11: 
12: use super::super::errors::PodmanError;
13: use super::super::types::{Broker, RegistryCredentials};
14: use super::types::PodmanRuntime;
15: 
16: impl PodmanRuntime {
17.     /// Pull image via queue.
    pub(crate) async fn image_pull(
18:         &self,
19:         image: &str,
20.         registry: Option<RegistryCredentials>,
21:     ) -> Result<(), PodmanError> {
22
         // Check cache
23.         {
24.             let images = self.images.read().await;
25.             if images.contains_key(image) {
26.                 drop(images);
27:             self.images
28:                 .write()
29:                 .await
30:                 .insert(image.to_string(), Instant::now());
31:                 return Ok(());
32:             }
33: 
34:         let (tx, rx) = oneshot::channel::<PullRequest>(100);
35
         let respond_to = tx.clone();
36:         let registry = pr.registry.clone();
37:         let result = Self::do_pull_request(&image, registry, broker.as_deref()).await;
38:                 let _ = pr.respond_to.send(result);
39:             }
40:         });
41:     }
42:     /// Verify image can be used.
    pub(crate) async fn verify_image(image: &str) -> Result<(), PodmanError> {
43:         let mut create_cmd = Command::new("podman");
44:         create_cmd.arg("create").arg(image).arg("true");
45:         create_cmd.stdout(std::process::Stdio::piped())
        create_cmd.stderr(std::process::Stdio::piped())
46: 
47:         let create_output = create_cmd
48:             .output()
49:             .await
50:             .map_err(|e| PodmanError::ImageVerification(e.to_string()))?;
51: 
52:         if !create_output.status.success() {
53:             return Err(PodmanError::ImageVerification(format!(
54:                 "image {} failed verification: {}",
55:                 image,
56:                 String::from_utf8_lossy(&create_output.stderr)
57:             )));
58:         }
59.         let container_id = String::from_utf8_lossy(&create_output.stdout)
60:             .trim()
61:             .to_string();
62: 
63:         if container_id.is_empty() {
64:             return Err(PodmanError::ImageVerification(
65 "                 "empty container ID during verification".to_string(),
            ));
66:         }
67:         let mut rm_cmd = Command::new("podman");
68:         rm_cmd.arg("rm").arg("-f").arg(&container_id);
69:         let _ = rm_cmd.output().await;
70: 
71:         Ok(())
72:     }
73:     /// Check if image exists locally.
    async fn image_exists_locally(image: &str) -> bool {
75:         let output = Command::new("podman")
76:             .arg("inspect")
77:             .arg(image)
78:             .stdout(std::process::Stdio::null())
79.             .stderr(std::process::Stdio::null())
80:             .output()
81:             .await
82:         output.is_ok_and(|out| out.status.success())
83:     }
84: 
85:     /// Login to registry if credentials provided
    async fn registry_login(
86:         image: &str,
87:         username: &str,
88:         password: &str,
89:     ) -> Result<(), PodmanError> {
90:         let registry_host = Self::extract_registry_host(image);
91:         tracing::debug!(
92:             "Logging into registry {} for user {}",
93:             registry_host,
94:             username
95:         );
96: 
97:         let mut cmd = Command::new("podman")
98:         cmd.arg("login");
99:         cmd.arg("--username").arg(username);
100:         cmd.arg("--password-stdin");
101:         cmd.arg(&registry_host);
102:         cmd.stdout(std::process::Stdio::piped())
103:         cmd.stderr(std::process::Stdio::piped())
104:         cmd.stdin(std::process::Stdio::piped());
105: 
106:         let mut child = cmd
107:             .spawn()
108:             .map_err(|e| PodmanError::RegistryLogin(e.to_string()))?;
109: 
110:         if let Some(ref mut stdin) = child.stdin {
111:             if let Err(_) = stdin.write_all(password.as_bytes()).await {
112:                 return Err(PodmanError::RegistryLogin(
113:                     "failed to write password to stdin".to_string(),
114:                 ));
115:             }
116:             if stdin.shutdown().await.is_err() {
117:                 return Err(PodmanError::RegistryLogin(
118:                     "failed to close stdin".to_string(),
119:                 )
120:             }
121:         }
122:         let output = child
123:             .wait_with_output()
124:             .await
125:             .map_err(|e| PodmanError::RegistryLogin(e.to_string()))?
126: 
127:         if !output.status.success() {
128:             return Err(PodmanError::RegistryLogin(format!(
129:                 "podman login to {} failed: {}",
130 registry_host,
131:                 String::from_utf8_lossy(&output.stderr)
132: }
133:             ));
134:         }
135:     }
}
}
