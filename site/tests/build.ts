/** Helpers to build the site into a scratch directory with fixture env. */
export async function buildSite(
  outDir: string,
  env: Record<string, string>,
): Promise<void> {
  const proc = Bun.spawn(["bun", "run", "astro", "build", "--outDir", outDir], {
    env: { ...process.env, GITHUB_TOKEN: "", ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  const code = await proc.exited;
  if (code !== 0) {
    throw new Error(
      `astro build failed (${code}): ${await new Response(proc.stderr).text()}`,
    );
  }
}
