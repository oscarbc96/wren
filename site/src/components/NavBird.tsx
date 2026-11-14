import { useId } from "react";
import { navBirdSvg } from "@/lib/nav-bird.generated";

/**
 * The top-bar bird, inline. Fumadocs renders the nav title more than once
 * (mobile bar and sidebar); a hidden copy's gradient and mask ids would hijack
 * the visible one, so every instance gets its own id prefix.
 */
export function NavBird() {
  const prefix = `nb${useId().replace(/[^a-zA-Z0-9]/g, "")}-`;
  return (
    <>
      <span
        className="wren-nav-bird"
        dangerouslySetInnerHTML={{
          __html: navBirdSvg.replaceAll("nb-", prefix),
        }}
      />
      <span className="sr-only">Wren</span>
    </>
  );
}
