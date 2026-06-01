use crate::kernel::authority::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CapabilitySet {
    FileSystemRead,
    FileSystemWrite,
    PatchFunction,
    SpawnWorker,
}

pub struct KernelBoundary;

impl KernelBoundary {
    pub fn validate<T>(req: &KernelRequest<T>) -> Result<(), String> {
        if let Some(exp) = req.capability.expires_at {
            if std::time::SystemTime::now() > exp { return Err("CAPABILITY_EXPIRED".into()); }
        }
        if req.identity.actor_id != req.capability.bound_actor {
            return Err("IDENTITY_MISMATCH".into());
        }
        Ok(())
    }

    pub async fn request<T, F, R>(req: KernelRequest<T>, dispatch: F) -> Result<R, String>
    where F: FnOnce(KernelRequest<T>) -> R {
        Self::validate(&req)?;
        println!("[KERNEL_AUDIT] Dispatching trace: {}", req.lineage.trace_id);
        Ok(dispatch(req))
    }
}
