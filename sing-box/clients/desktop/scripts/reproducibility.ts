import { sync as spawnSync } from "cross-spawn";

const sourceDateEpochPattern = /^(0|[1-9][0-9]*)$/u;

function commandOutput(
  command: string,
  commandArguments: string[],
  workingDirectory: string,
): string {
  const result = spawnSync(command, commandArguments, {
    cwd: workingDirectory,
    encoding: "utf-8",
  });
  if (result.error) {
    throw new Error(`${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`${command} exited with code ${result.status ?? 1}`);
  }
  return result.stdout.trim();
}

function gitCommitEpoch(workingDirectory: string): number {
  const value = commandOutput(
    "git",
    ["log", "-1", "--format=%ct"],
    workingDirectory,
  );
  if (!sourceDateEpochPattern.test(value)) {
    throw new Error(`git returned an invalid commit timestamp: ${value}`);
  }
  return Number(value);
}

export function configureReproducibleBuild(
  sourceDirectories: string[],
): string {
  let sourceDateEpoch = process.env.SOURCE_DATE_EPOCH;
  if (sourceDateEpoch === undefined) {
    sourceDateEpoch = String(
      Math.max(...Array.from(new Set(sourceDirectories), gitCommitEpoch)),
    );
  } else if (!sourceDateEpochPattern.test(sourceDateEpoch)) {
    throw new Error(`invalid SOURCE_DATE_EPOCH: ${sourceDateEpoch}`);
  }
  process.env.SOURCE_DATE_EPOCH = sourceDateEpoch;
  process.env.TZ = "UTC";
  process.env.LANG = "C";
  process.env.LC_ALL = "C";
  return sourceDateEpoch;
}
