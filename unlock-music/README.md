# Unlock Music 音乐解锁
**由于DMCA Takedown，暂时移除仓库所有代码以及Commits**

- 项目新域名：[unlock-music.dev](https://unlock-music.dev)
- 获取更多信息，欢迎加入 Telegram 群组 [`@unlock_music_chat`][tg_group]！
- Unlock Music 项目是以学习和技术研究的初衷创建的，修改、再分发时请遵循 [License][license]
- Unlock Music 的 CLI 版本可以在 [unlock-music/cli][repo_cli] 找到，大批量转换建议使用 CLI 版本。
- [相关的其他项目][related_projects]

![Test Build](https://github.com/unlock-music/unlock-music/workflows/Test%20Build/badge.svg)
![GitHub releases](https://img.shields.io/github/downloads/unlock-music/unlock-music/total)
![Docker Pulls](https://img.shields.io/docker/pulls/ix64/unlock-music)

[license]: https://github.com/unlock-music/unlock-music/blob/master/LICENSE

[repo_cli]: https://github.com/unlock-music/cli

[tg_group]: https://t.me/unlock_music_chat

[related_projects]: https://github.com/unlock-music/unlock-music/wiki/和UnlockMusic相关的项目

## 使用方法

### 安装浏览器扩展

[![Chrome Web Store](https://storage.googleapis.com/chrome-gcs-uploader.appspot.com/image/WlD8wC6g8khYWPJUsQceQkhXSlv1/UV4C4ybeBTsZt43U4xis.png)](https://chrome.google.com/webstore/detail/gldlhhhmienbhlpkfanjpmffdjblmegd)
[<img src="https://developer.microsoft.com/en-us/store/badges/images/Chinese_Simplified_get-it-from-MS.png" height="60" alt="Microsoft Edge Addons"/>](https://microsoftedge.microsoft.com/addons/detail/ggafoipegcmodfhakdkalpdpcdkiljmd)
[![Firefox Browser Addons](https://ffp4g1ylyit3jdyti1hqcvtb-wpengine.netdna-ssl.com/addons/files/2015/11/get-the-addon.png)](https://addons.mozilla.org/zh-CN/firefox/addon/unlock-music/)

### 使用已构建版本

- 从[GitHub Release](https://github.com/unlock-music/unlock-music/releases/latest)下载已构建的版本
    - 本地使用请下载`legacy版本`（`modern版本`只能通过 **http(s)协议** 访问）
- 解压缩后即可部署或本地使用（**请勿直接运行源代码**）

### 使用 Docker 镜像

```shell
docker run --name unlock-music -d -p 8080:80 ix64/unlock-music
```

### 自行构建

- 环境要求
    - nodejs (v16.x)
    - npm

1. 获取项目源代码后安装相关依赖：

   ```sh
   npm ci
   ```

2. 然后进行构建。编译后的文件保存到 dist 目录下：

   ```sh
   npm run build
   ```

- 如果是用于开发，可以执行 `npm run serve`。

3. 如需构建浏览器扩展，build 完成后还需要执行：

   ```sh
   npm run make-extension
   ```
