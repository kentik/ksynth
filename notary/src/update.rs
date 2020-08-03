use std::env;
use std::ffi::{CStr, CString, OsString};
use std::fs::{self, File, Metadata};
use std::time::Duration;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::thread;
use anyhow::{anyhow, Result};
use log::{debug, error, info};
use nix::unistd;
use tokio::time::Interval;
use super::{Artifact, Client, Item, Query};

pub struct Updates {
    client: Client,
    query:  Query,
}

pub struct Update {
    artifact:   Artifact,
    arguments:  Vec<CString>,
    executable: PathBuf,
    metadata:   Metadata,
    temporary:  PathBuf,
}

impl Updates {
    pub fn new(client: Client, query: Query) -> Self {
        Self { client, query }
    }

    pub async fn watch(&self, interval: &mut Interval) -> Item {
        loop {
            interval.tick().await;

            match self.client.latest(&self.query).await {
                Ok(Some(item)) => return item,
                Ok(None)       => debug!("no updates available"),
                Err(e)         => error!("notary error: {:?}", e),
            }
        }
    }

    pub async fn fetch(&self, item: Item) -> Result<Update> {
        let Item { artifact: Artifact { name, version, .. }, .. } = &item;

        let exe  = env::current_exe()?;
        let meta = fs::metadata(&exe)?;

        if meta.permissions().readonly() {
            return Err(anyhow!("{:?} is not writable", exe));
        }

        let dir = match exe.parent() {
            Some(dir) => dir.to_path_buf(),
            None      => unistd::getcwd()?,
        };

        let tmp = format!("{}-update-{}", name, version);
        let tmp = dir.join(tmp);

        debug!("download to {:?}", tmp);

        let mut file = File::create(&tmp)?;
        self.client.stream(&item, &mut file).await?;
        file.sync_all()?;

        debug!("download complete");

        let args = env::args().map(|arg| {
            Ok(CString::new(arg)?)
        }).collect::<Result<Vec<_>>>()?;

        Ok(Update {
            artifact:   item.artifact,
            arguments:  args,
            executable: exe,
            metadata:   meta,
            temporary:  tmp,
        })
    }
}

impl Update {
    pub fn apply(self, retry: Duration) -> Result<()> {
        let exe  = self.executable.as_os_str().to_owned();
        let args = self.arguments.iter().map(|str| {
            str.as_c_str()
        }).collect::<Vec<_>>();

        fs::remove_file(&self.executable)?;

        loop {
            if let Err(e) = self.exec(&exe, &args) {
                error!("failed to exec {:?}: {:?}", exe, e);
                thread::sleep(retry);
            }
        }
    }

    fn exec(&self, exe: &OsString, args: &[&CStr]) -> Result<()> {
        let Artifact { name, version, .. } = &self.artifact;

        let path  = CString::new(exe.as_bytes())?;
        let perms = self.metadata.permissions();

        fs::copy(&self.temporary, exe)?;
        fs::set_permissions(exe, perms)?;
        fs::remove_file(&self.temporary)?;

        info!("execute {} {}", name, version);

        unistd::execv(&path, args)?;

        Ok(())
    }
}
