import type { Rectangle } from "electron";

const MAIN_WINDOW_DEFAULT_WIDTH = 960;
const MAIN_WINDOW_DEFAULT_HEIGHT = 640;
export const MAIN_WINDOW_MINIMUM_WIDTH = 480;
export const MAIN_WINDOW_MINIMUM_HEIGHT = 320;

export interface MainWindowState extends Rectangle {
  maximized: boolean;
}

export function parseMainWindowState(value: unknown): MainWindowState | undefined {
  if (typeof value !== "object" || value === null) {
    return undefined;
  }
  const state = value as Record<string, unknown>;
  if (
    !Number.isSafeInteger(state.x) ||
    !Number.isSafeInteger(state.y) ||
    !Number.isSafeInteger(state.width) ||
    !Number.isSafeInteger(state.height) ||
    (state.width as number) <= 0 ||
    (state.height as number) <= 0 ||
    typeof state.maximized !== "boolean"
  ) {
    return undefined;
  }
  return {
    x: state.x as number,
    y: state.y as number,
    width: state.width as number,
    height: state.height as number,
    maximized: state.maximized,
  };
}

function constrain(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

export function restoredMainWindowBounds(
  state: MainWindowState | undefined,
  workAreas: Rectangle[],
  primaryWorkArea: Rectangle,
): Rectangle {
  let targetWorkArea = primaryWorkArea;
  if (state !== undefined) {
    let largestIntersection = 0;
    for (const workArea of workAreas) {
      const intersectionWidth = Math.max(
        0,
        Math.min(state.x + state.width, workArea.x + workArea.width) -
          Math.max(state.x, workArea.x),
      );
      const intersectionHeight = Math.max(
        0,
        Math.min(state.y + state.height, workArea.y + workArea.height) -
          Math.max(state.y, workArea.y),
      );
      const area = intersectionWidth * intersectionHeight;
      if (area > largestIntersection) {
        largestIntersection = area;
        targetWorkArea = workArea;
      }
    }
  }

  const requestedWidth = state?.width ?? MAIN_WINDOW_DEFAULT_WIDTH;
  const requestedHeight = state?.height ?? MAIN_WINDOW_DEFAULT_HEIGHT;
  const minimumWidth = Math.min(MAIN_WINDOW_MINIMUM_WIDTH, targetWorkArea.width);
  const minimumHeight = Math.min(MAIN_WINDOW_MINIMUM_HEIGHT, targetWorkArea.height);
  const width = constrain(requestedWidth, minimumWidth, targetWorkArea.width);
  const height = constrain(requestedHeight, minimumHeight, targetWorkArea.height);
  const requestedX =
    state?.x ?? targetWorkArea.x + Math.round((targetWorkArea.width - width) / 2);
  const requestedY =
    state?.y ?? targetWorkArea.y + Math.round((targetWorkArea.height - height) / 2);

  return {
    x: constrain(requestedX, targetWorkArea.x, targetWorkArea.x + targetWorkArea.width - width),
    y: constrain(requestedY, targetWorkArea.y, targetWorkArea.y + targetWorkArea.height - height),
    width,
    height,
  };
}
