import React from "react";
import { AbsoluteFill, Sequence } from "remotion";
import { Scene1Brand } from "./Scene1Brand";
import { Scene2CloudDrives } from "./Scene2CloudDrives";
import { Scene3MediaServer } from "./Scene3MediaServer";
import { Scene4MediaLibrary } from "./Scene4MediaLibrary";
import { Scene5BookLibrary } from "./Scene5BookLibrary";
import { Scene6MusicPlayer } from "./Scene6MusicPlayer";
import { Scene7Download } from "./Scene7Download";
import { Scene8CLI } from "./Scene8CLI";
import { Scene9CTA } from "./Scene9CTA";

// ============ TIMING (60s = 1800 frames @ 30fps) ============
// Scene 1: Brand         0    - 150   (5s)   品牌开场
// Scene 2: Cloud Drives  150  - 390   (8s)   多网盘
// Scene 3: Media Server  390  - 600   (7s)   媒体服务器
// Scene 4: Media Library 600  - 810   (7s)   媒体刮削
// Scene 5: Book Library  810  - 1020  (7s)   书籍库
// Scene 6: Music Player  1020 - 1230  (7s)   粒子播放器
// Scene 7: Download      1230 - 1440  (7s)   下载引擎
// Scene 8: CLI           1440 - 1620  (6s)   CLI插件
// Scene 9: CTA           1620 - 1800  (6s)   结尾CTA
// =============================================================

export const BoxPlayerFullPromoComposition: React.FC = () => {
  return (
    <AbsoluteFill style={{ backgroundColor: "#0B0E1A" }}>
      <Sequence from={0} durationInFrames={150}>
        <Scene1Brand />
      </Sequence>
      <Sequence from={150} durationInFrames={240}>
        <Scene2CloudDrives />
      </Sequence>
      <Sequence from={390} durationInFrames={210}>
        <Scene3MediaServer />
      </Sequence>
      <Sequence from={600} durationInFrames={210}>
        <Scene4MediaLibrary />
      </Sequence>
      <Sequence from={810} durationInFrames={210}>
        <Scene5BookLibrary />
      </Sequence>
      <Sequence from={1020} durationInFrames={210}>
        <Scene6MusicPlayer />
      </Sequence>
      <Sequence from={1230} durationInFrames={210}>
        <Scene7Download />
      </Sequence>
      <Sequence from={1440} durationInFrames={180}>
        <Scene8CLI />
      </Sequence>
      <Sequence from={1620} durationInFrames={180}>
        <Scene9CTA />
      </Sequence>
    </AbsoluteFill>
  );
};
