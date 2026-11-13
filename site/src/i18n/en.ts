import type { Ui } from "@/i18n/ui";

export const en: Ui = {
  nav: {
    main: "Main",
    docs: "Docs",
    changelog: "Changelog",
    download: "Downloads",
    github: "GitHub",
  },
  footer: {
    license: "Wren is open source under MPL-2.0.",
    source: "Source on GitHub",
    privacy: "Privacy",
    llms: "llms.txt",
  },
  docsTitle: "Documentation",
  language: "Language",
  home: {
    title: "Wren: The light browser that works with you.",
    description:
      "Wren is an open-source desktop browser in Rust on Servo. It aims to use half the memory of Chrome, run Chrome extensions, and bring an AI agent you control.",
    kicker: "Pre-alpha: in design. Nothing to download yet.",
    heading: "The light browser that works with you.",
    lede: "Wren is an open-source desktop browser written in Rust on the Servo engine. It aims to use half the memory of Chrome, run Chrome extensions, and bring an AI agent you control.",
    docsButton: "Roadmap",
    sourceButton: "GitHub repo",
    designButton: "Read the design",
    featuresLabel: "What Wren aims to do",
    features: [
      {
        title: "Light by design",
        text: "Rust and Servo, tab sleeping, built-in ad blocking, and RAM published as a benchmark against Chrome.",
      },
      {
        title: "Works like Chrome",
        text: "Chrome extensions, Chrome DevTools, password managers and passkeys, WebAssembly.",
      },
      {
        title: "AI-native",
        text: "Bring your own model or local Ollama, an agent that reads and acts with your confirmation, WebMCP, generated answer pages that stay live, and routines like 'my usual Friday order' that always ask before paying.",
      },
      {
        title: "Open source",
        text: "MPL-2.0, no telemetry, local-first.",
      },
    ],
    downloadTitle: "Download",
    downloadText:
      "Wren is still in design and there is nothing to download yet. Watch GitHub Releases for the first build.",
    releasesButton: "GitHub Releases",
    noRelease: "No release has been published yet.",
    changelogButton: "Changelog",
    roadmapButton: "Roadmap",
  },
  changelog: {
    title: "Changelog | Wren",
    description:
      "Release notes for every published version of Wren, from GitHub Releases.",
    heading: "Changelog",
    notesNote: "Release notes are published in English on GitHub Releases.",
    empty: "No releases yet",
    emptyText:
      "Nothing has been published yet. Releases will appear here as soon as the first one ships.",
    prerelease: "Pre-release",
    pending: "Pending release",
    unreleased: "Unreleased",
    viewPr: "View the release pull request",
    newer: "Newer",
    older: "Older",
    dateLocale: "en-US",
  },
  downloads: {
    title: "Downloads | Wren",
    description:
      "Wren is pre-alpha and has no builds yet. Build it from source, or watch GitHub Releases for the first build.",
    heading: "Downloads",
    lede: "Wren is pre-alpha and in design. There are no builds to download yet.",
    status: "Pre-alpha",
    platformTitle: "macOS 14+ on Apple Silicon",
    platformText:
      "The first builds will target macOS 14 or later on Apple Silicon (M1 and newer). Other platforms are not planned yet.",
    soonButton: "Coming soon",
    buildTitle: "Build from source",
    buildText:
      "You can build and run the current shell yourself with the Rust toolchain.",
    buildLink: "Build from source",
    releasesTitle: "GitHub Releases",
    releasesText:
      "Builds will be published on GitHub Releases as soon as there is one to try.",
    releasesLink: "Open GitHub Releases",
  },
  notFound: {
    title: "Page not found | Wren",
    description:
      "This page does not exist. Go back to the Wren home page or the docs.",
    heading: "Page not found",
    text: "The page you asked for does not exist or has moved.",
    home: "Back to the home page",
  },
  starsLabel: "GitHub, {count} stars",
  translations: {},
};
