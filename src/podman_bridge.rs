use bollard::Docker;
use bollard::container::{CreateContainerOptions, Config};
use std::process::Command;
use anyhow::Result;

pub struct PodmanCortex {
    pub api: Docker,
}

impl PodmanCortex {
    pub fn connect() -> Result<Self> {
        let socket_path = "unix:///run/user/1000/podman/podman.sock";
        let api = Docker::connect_with_socket(socket_path, 120, bollard::API_DEFAULT_VERSION)?;
        Ok(Self { api })
    }

    pub async fn spin_up_alpine(&self) -> Result<String> {
        let image_name = "docker.io/library/alpine:latest";
        let _ = Command::new("podman").arg("pull").arg(image_name).output();
        let config = Config { image: Some(image_name), cmd: Some(vec!["sleep", "300"]), ..Default::default() };
        let _ = self.api.remove_container("nexus_test_node", Some(bollard::container::RemoveContainerOptions { force: true, ..Default::default() })).await;
        let id = self.api.create_container(Some(CreateContainerOptions { name: "nexus_test_node", platform: None::<&str> }), config).await?.id;
        self.api.start_container::<String>(&id, None).await?;
        Ok(format!("Deployed Alpine Test Node [{}]", &id[0..8]))
    }

    pub async fn build_and_run(&self) -> Result<String> {
        // 1. The Ironclad Build: Use the native OS to build the Dockerfile
        let build_status = Command::new("podman")
            .args(&["build", "-t", "nexus_dynamic", "."])
            .output()?;

        if !build_status.status.success() {
            let err = String::from_utf8_lossy(&build_status.stderr);
            return Err(anyhow::anyhow!("Build Failed: {}", err));
        }

        // 2. Clear out any old versions
        let _ = self.api.remove_container("nexus_dynamic_node", Some(bollard::container::RemoveContainerOptions { force: true, ..Default::default() })).await;

        // 3. Ignite the new custom container
        let config = Config {
            image: Some("nexus_dynamic"),
            ..Default::default()
        };

        let id = self.api.create_container(
            Some(CreateContainerOptions { 
                name: "nexus_dynamic_node",
                platform: None::<&str>,
            }),
            config,
        ).await?.id;

        self.api.start_container::<String>(&id, None).await?;
        Ok(format!("Successfully Built and Deployed Custom Node [{}]", &id[0..8]))
    }
}
