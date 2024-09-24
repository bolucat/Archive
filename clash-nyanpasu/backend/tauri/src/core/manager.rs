use std::borrow::Cow;

/// 给clash内核的tun模式授权
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn grant_permission(core: &nyanpasu_utils::core::CoreType) -> anyhow::Result<()> {
    use std::process::Command;

    let path = crate::core::clash::core::find_binary_path(&core)
        .map_err(|_| anyhow::anyhow!("clash core not found"))?
        .canonicalize()?
        .to_string_lossy()
        .to_string();

    log::debug!("grant_permission path: {:?}", path);

    #[cfg(target_os = "macos")]
    let output = {
        // the path of clash /Applications/Clash Nyanpasu.app/Contents/MacOS/clash
        // https://apple.stackexchange.com/questions/82967/problem-with-empty-spaces-when-executing-shell-commands-in-applescript
        // let path = escape(&path);
        let path = path.replace(' ', "\\\\ ");
        let shell = format!("chown root:admin {path}\nchmod +sx {path}");
        let command = format!(r#"do shell script "{shell}" with administrator privileges"#);
        Command::new("osascript")
            .args(vec!["-e", &command])
            .output()?
    };

    #[cfg(target_os = "linux")]
    let output = {
        let path = path.replace(' ', "\\ "); // 避免路径中有空格
        let shell = format!("setcap cap_net_bind_service,cap_net_admin=+ep {path}");

        let sudo = match Command::new("which").arg("pkexec").output() {
            Ok(output) => {
                if output.stdout.is_empty() {
                    "sudo"
                } else {
                    "pkexec"
                }
            }
            Err(_) => "sudo",
        };

        Command::new(sudo).arg("sh").arg("-c").arg(shell).output()?
    };

    if output.status.success() {
        Ok(())
    } else {
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        anyhow::bail!("{stderr}");
    }
}

#[allow(unused)]
pub fn escape(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();

    let mut owned = None;

    for pos in 0..bytes.len() {
        let special = match bytes[pos] {
            b' ' => Some(b' '),
            _ => None,
        };
        if let Some(s) = special {
            if owned.is_none() {
                owned = Some(bytes[0..pos].to_owned());
            }
            owned.as_mut().unwrap().push(b'\\');
            owned.as_mut().unwrap().push(b'\\');
            owned.as_mut().unwrap().push(s);
        } else if let Some(owned) = owned.as_mut() {
            owned.push(bytes[pos]);
        }
    }

    if let Some(owned) = owned {
        Cow::Owned(String::from_utf8(owned).unwrap())
    } else {
        Cow::Borrowed(std::str::from_utf8(bytes).unwrap())
    }
}
