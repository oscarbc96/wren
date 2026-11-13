import type { Ui } from "@/i18n/ui";

export const es: Ui = {
  nav: {
    main: "Principal",
    docs: "Documentación",
    changelog: "Novedades",
    download: "Descargar",
    github: "GitHub",
  },
  footer: {
    license: "Wren es código abierto bajo MPL-2.0.",
    source: "Código en GitHub",
    privacy: "Privacidad",
    llms: "llms.txt",
  },
  docsTitle: "Documentación",
  language: "Idioma",
  home: {
    title: "Wren: el navegador ligero que trabaja contigo.",
    description:
      "Wren es un navegador de escritorio de código abierto en Rust sobre Servo. Pretende usar la mitad de memoria que Chrome y traer un agente de IA que tú controlas.",
    kicker: "Pre-alfa: en diseño. Todavía no hay nada que descargar.",
    heading: "El navegador ligero que trabaja contigo.",
    lede: "Wren es un navegador de escritorio de código abierto escrito en Rust sobre el motor Servo. Pretende usar la mitad de memoria que Chrome, ejecutar extensiones de Chrome y ofrecer un agente de IA que tú controlas.",
    docsButton: "Hoja de ruta",
    sourceButton: "Repositorio en GitHub",
    designButton: "Leer el diseño",
    featuresLabel: "A qué aspira Wren",
    features: [
      {
        title: "Ligero por diseño",
        text: "Rust y Servo, pestañas que duermen, bloqueo de anuncios integrado y RAM publicada como punto de referencia frente a Chrome.",
      },
      {
        title: "Funciona como Chrome",
        text: "Extensiones de Chrome, Chrome DevTools, gestores de contraseñas y passkeys, WebAssembly.",
      },
      {
        title: "Nativo para IA",
        text: "Trae tu propio modelo o un Ollama local, un agente que lee y actúa con tu confirmación, WebMCP, páginas de respuesta generadas que siguen vivas y rutinas como 'mi pedido habitual del viernes' que siempre preguntan antes de pagar.",
      },
      {
        title: "Código abierto",
        text: "MPL-2.0, sin telemetría, local primero.",
      },
    ],
    downloadTitle: "Descargar",
    downloadText:
      "Wren sigue en diseño y todavía no hay nada que descargar. Sigue las versiones en GitHub para conocer la primera compilación.",
    releasesButton: "Versiones en GitHub",
    noRelease: "Todavía no se ha publicado ninguna versión.",
    changelogButton: "Novedades",
    roadmapButton: "Hoja de ruta",
  },
  changelog: {
    title: "Novedades | Wren",
    description:
      "Notas de cada versión publicada de Wren, tomadas de las versiones de GitHub.",
    heading: "Novedades",
    notesNote:
      "Las notas de cada versión se publican en inglés en las versiones de GitHub.",
    empty: "Todavía no hay versiones",
    emptyText:
      "Aún no se ha publicado nada. Las versiones aparecerán aquí en cuanto salga la primera.",
    prerelease: "Versión preliminar",
    newer: "Más recientes",
    older: "Anteriores",
    dateLocale: "es-ES",
  },
  notFound: {
    title: "Página no encontrada | Wren",
    description:
      "Esta página no existe. Vuelve a la página de inicio de Wren o a la documentación.",
    heading: "Página no encontrada",
    text: "La página que buscas no existe o ha cambiado de sitio.",
    home: "Volver al inicio",
  },
  starsLabel: "GitHub, {count} estrellas",
  translations: {
    "Choose a language(language switcher)": "Elige un idioma",
    "Choose a language(language switcher)(aria-label)": "Elige un idioma",
    "Search(search dialog)": "Buscar",
    "Search(search trigger)": "Buscar",
    "Open Search(search trigger)(aria-label)": "Abrir búsqueda",
    "Close Search(search dialog)(aria-label)": "Cerrar búsqueda",
    "On this page(table of contents)": "En esta página",
    "No Headings(table of contents)": "Sin encabezados",
    "Next Page(pagination)": "Página siguiente",
    "Previous Page(pagination)": "Página anterior",
    "Toggle Menu(mobile menu)(aria-label)": "Alternar menú",
    "Toggle Theme(theme switcher)(aria-label)": "Cambiar tema",
    "Light(theme switcher)(aria-label)": "Claro",
    "Dark(theme switcher)(aria-label)": "Oscuro",
    "System(theme switcher)(aria-label)": "Sistema",
    "Open Sidebar(sidebar)(aria-label)": "Abrir barra lateral",
    "Close Sidebar(sidebar)(aria-label)": "Cerrar barra lateral",
    "Collapse Sidebar(sidebar)(aria-label)": "Contraer barra lateral",
    "Skip to Content(skip link)": "Saltar al contenido",
    "Copy Text(code block)(aria-label)": "Copiar texto",
    "Copied Text(code block)(aria-label)": "Texto copiado",
  },
};
