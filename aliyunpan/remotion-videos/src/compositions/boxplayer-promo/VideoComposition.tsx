import React from "react";
import { AbsoluteFill, Sequence } from "remotion";
import { Scene1Hook } from "./Scene1Hook";
import { Scene2Identity } from "./Scene2Identity";
import { Scene3CloudDrives } from "./Scene3CloudDrives";
import { Scene4Features } from "./Scene4Features";
import { Scene5CLI } from "./Scene5CLI";
import { Scene6CTA } from "./Scene6CTA";

// ============ TIMING ============
// Total: 900 frames = 30 seconds at 30fps
// Scene 1: Hook      0-120   (4s)
// Scene 2: Identity  120-210 (3s)
// Scene 3: Clouds    210-390 (6s)
// Scene 4: Features  390-660 (9s)
// Scene 5: CLI       660-780 (4s)
// Scene 6: CTA       780-900 (4s)
// ============================================

export const BoxPlayerPromoComposition: React.FC = () => {
  return (
    <AbsoluteFill style={{ backgroundColor: "#0B0E1A" }}>
      <Sequence from={0} durationInFrames={120}>
        <Scene1Hook />
      </Sequence>
      <Sequence from={120} durationInFrames={90}>
        <Scene2Identity />
      </Sequence>
      <Sequence from={210} durationInFrames={180}>
        <Scene3CloudDrives />
      </Sequence>
      <Sequence from={390} durationInFrames={270}>
        <Scene4Features />
      </Sequence>
      <Sequence from={660} durationInFrames={120}>
        <Scene5CLI />
      </Sequence>
      <Sequence from={780} durationInFrames={120}>
        <Scene6CTA />
      </Sequence>
    </AbsoluteFill>
  );
};
