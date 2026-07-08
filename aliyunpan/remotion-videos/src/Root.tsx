import React from "react";
import { Composition } from "remotion";
import { BoxPlayerFullPromoComposition } from "./compositions/boxplayer-full/VideoComposition";
import { BoxPlayerScreenshotsComposition } from "./compositions/boxplayer-screenshots/VideoComposition";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="boxplayer-full"
        component={BoxPlayerFullPromoComposition}
        durationInFrames={1800}
        fps={30}
        width={1920}
        height={1080}
        defaultProps={{}}
      />
      <Composition
        id="boxplayer-screenshots"
        component={BoxPlayerScreenshotsComposition}
        durationInFrames={1800}
        fps={30}
        width={1920}
        height={1080}
        defaultProps={{}}
      />
    </>
  );
};
