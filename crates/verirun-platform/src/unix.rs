//! The Unix process group: the child leads its own group (the
//! runner arranges that at spawn through `process_group(0)`),
//! and termination signals the whole group through the shell's
//! builtin `kill`.

use std::io;
use std::process::{Child, Command, Stdio};

/// A Unix process group, identified by its leader's process id.
///
/// The group itself is created by the spawn configuration, so
/// attaching only records the leader. Termination sends KILL to
/// the group through the shell builtin: the standalone `kill`
/// utility's negative-pid form was observed to be a silent no-op
/// in the development sandbox, while the builtin reaches the
/// syscall directly. A failed shell spawn is silent by design;
/// the caller follows termination with a direct kill of the
/// leader as a fallback.
pub struct ProcessGroup {
    /// The group leader's process id, once a child is attached.
    leader: Option<u32>,
}

impl ProcessGroup {
    /// Creates an empty group with no leader attached.
    #[must_use]
    pub const fn new() -> Self {
        Self { leader: None }
    }

    /// Records `child` as the group's leader. The child was
    /// spawned as a group leader by the runner, so the group
    /// already exists; this cannot fail on Unix.
    ///
    /// # Errors
    ///
    /// Never returns an error on Unix. The result is part of the
    /// cross-platform API shape, where attaching can fail.
    pub fn attach(&mut self, child: &Child) -> io::Result<()> {
        self.leader = Some(child.id());
        Ok(())
    }

    /// Kills every process in the group with KILL. Does nothing
    /// when no child was attached.
    pub fn terminate(&self) {
        if let Some(leader) = self.leader {
            let script = format!("kill -KILL -{leader}");
            let _ = Command::new("/bin/sh")
                .args(["-c", &script])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

impl Default for ProcessGroup {
    fn default() -> Self {
        Self::new()
    }
}
