/** Shape of the UI strings every locale provides. */
export interface Ui {
  nav: {
    main: string;
    docs: string;
    changelog: string;
    download: string;
    github: string;
  };
  footer: { license: string; source: string; privacy: string; llms: string };
  docsTitle: string;
  language: string;
  home: {
    title: string;
    description: string;
    kicker: string;
    heading: string;
    lede: string;
    docsButton: string;
    sourceButton: string;
    designButton: string;
    roadmapButton: string;
    featuresLabel: string;
    features: { title: string; text: string }[];
    downloadTitle: string;
    downloadText: string;
    releasesButton: string;
    noRelease: string;
    changelogButton: string;
  };
  changelog: {
    title: string;
    description: string;
    heading: string;
    /** Release notes are always English. */
    notesNote: string;
    empty: string;
    emptyText: string;
    prerelease: string;
    pending: string;
    unreleased: string;
    viewPr: string;
    newer: string;
    older: string;
    /** BCP 47 tag used to format dates. */
    dateLocale: string;
  };
  downloads: {
    title: string;
    description: string;
    heading: string;
    lede: string;
    status: string;
    platformTitle: string;
    platformText: string;
    soonButton: string;
    buildTitle: string;
    buildText: string;
    buildLink: string;
    releasesTitle: string;
    releasesText: string;
    releasesLink: string;
  };
  notFound: {
    title: string;
    description: string;
    heading: string;
    text: string;
    home: string;
  };
  /** `{count}` is the compact star count. */
  starsLabel: string;
  /** Fumadocs UI strings, keyed by their English text. */
  translations: Record<string, string>;
}
