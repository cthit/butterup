use crate::FileList;
use crate::Opt;
use chrono::DateTime;
use ssh2::Session;
use std::collections::BTreeMap;
use std::io::Read;
use std::net::TcpStream;
use std::path::PathBuf;
use std::str::FromStr;

pub struct Remote {
    pub username: String,
    pub remote: String,
    pub path: PathBuf,
}

pub fn connect(opt: &Opt) -> anyhow::Result<Session> {
    info!(
        r#"connecting to {}@{}"#,
        opt.remote.username, opt.remote.remote,
    );

    let stream = TcpStream::connect(&opt.remote.remote)?;
    let mut session = Session::new()?;
    session.set_tcp_stream(stream);
    session.handshake()?;
    session.userauth_pubkey_file(
        &opt.remote.username,
        None,
        &opt.privkey,
        opt.privkey_pass.as_deref(),
    )?;
    if !session.authenticated() {
        anyhow::bail!("ssh not authenticated");
    }
    session.set_allow_sigpipe(true);

    Ok(session)
}

pub fn file_list(opt: &Opt, session: &Session) -> anyhow::Result<FileList> {
    let mut channel = session.channel_session()?;
    channel.exec(&format!(
        r#"ls -1NU "{}""#,
        opt.remote
            .path
            .to_str()
            .ok_or_else(|| anyhow::format_err!("path not utf-8"))?
    ))?;
    let mut output = String::new();
    channel.read_to_string(&mut output)?;

    let mut list = BTreeMap::new();
    for file in output.lines() {
        let date = DateTime::parse_from_rfc3339(file)?;
        list.insert(date, file.to_string());
    }

    Ok(list)
}

impl FromStr for Remote {
    type Err = anyhow::Error;

    // user@remote:path
    // remote = host:port
    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        let (s, path) = s
            .rsplit_once(':')
            .ok_or_else(|| anyhow::format_err!("Missing ...:path"))?;
        let path = PathBuf::from_str(path)?;

        let (username, remote) = s
            .split_once('@')
            .ok_or_else(|| anyhow::format_err!("Missing user@..."))?;

        Ok(Remote {
            username: username.to_string(),
            remote: remote.to_string(),
            path,
        })
    }
}
