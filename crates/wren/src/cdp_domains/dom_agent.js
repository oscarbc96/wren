(function () {
  if (window.__wrenDom) return;
  const nodes = new Map();
  let nextId = 2;
  function idFor(node) {
    if (!node) return 0;
    if (node.__wrenNodeId && nodes.has(node.__wrenNodeId)) return node.__wrenNodeId;
    const id = node === document ? 1 : nextId++;
    try { node.__wrenNodeId = id; } catch (_) {}
    nodes.set(id, typeof WeakRef !== "undefined" ? new WeakRef(node) : node);
    return id;
  }
  function get(id) {
    const v = nodes.get(id);
    if (!v) return null;
    const n = typeof WeakRef !== "undefined" && v instanceof WeakRef ? v.deref() : v;
    if (!n) nodes.delete(id);
    return n || null;
  }
  function attrs(node) {
    if (!node || !node.attributes) return [];
    const out = [];
    for (let i = 0; i < node.attributes.length; i++) {
      const a = node.attributes[i];
      out.push(a.name, a.value);
    }
    return out;
  }
  function desc(node, depth) {
    if (!node) return null;
    const id = idFor(node);
    const d = {
      nodeId: id,
      backendNodeId: id,
      nodeType: node.nodeType || 0,
      nodeName: node.nodeName || "",
      localName: (node.localName || "").toLowerCase ? (node.localName || "").toLowerCase() : (node.localName || ""),
      nodeValue: node.nodeValue == null ? "" : String(node.nodeValue),
      childNodeCount: node.childNodes ? node.childNodes.length : 0,
      attributes: node.nodeType === 1 ? attrs(node) : undefined,
    };
    if (node.nodeType === 9) d.documentURL = String(document.URL || "");
    if (depth !== 0 && node.childNodes && node.childNodes.length) {
      const want = depth < 0 ? -1 : depth - 1;
      d.children = [];
      for (let i = 0; i < node.childNodes.length; i++) {
        const c = desc(node.childNodes[i], want);
        if (c) d.children.push(c);
      }
    }
    return d;
  }
  function quad(rect) {
    return [rect.left, rect.top, rect.right, rect.top, rect.right, rect.bottom, rect.left, rect.bottom];
  }
  function ensureObjects() {
    if (!window.__wrenObjects) window.__wrenObjects = { map: {}, next: 1 };
    return window.__wrenObjects;
  }
  function putObject(obj) {
    const reg = ensureObjects();
    const id = "wren-obj-" + (reg.next++);
    reg.map[id] = obj;
    return id;
  }
  window.__wrenDom = {
    idFor, get, desc, putObject,
    getDocument: function (depth) { idFor(document); return { root: desc(document, depth == null ? 1 : depth) }; },
    querySelector: function (nodeId, selector) {
      const n = get(nodeId); if (!n || !n.querySelector) return { nodeId: 0 };
      const r = n.querySelector(selector); return { nodeId: r ? idFor(r) : 0 };
    },
    querySelectorAll: function (nodeId, selector) {
      const n = get(nodeId); if (!n || !n.querySelectorAll) return { nodeIds: [] };
      const list = n.querySelectorAll(selector); const out = [];
      for (let i = 0; i < list.length; i++) out.push(idFor(list[i]));
      return { nodeIds: out };
    },
    getOuterHTML: function (nodeId) {
      const n = get(nodeId); if (!n) return { outerHTML: "" };
      if (n.nodeType === 9) return { outerHTML: n.documentElement ? n.documentElement.outerHTML : "" };
      return { outerHTML: n.outerHTML != null ? String(n.outerHTML) : "" };
    },
    setOuterHTML: function (nodeId, outerHTML) {
      const n = get(nodeId); if (n && n.outerHTML != null) n.outerHTML = outerHTML; return {};
    },
    describeNode: function (nodeId, depth) {
      const n = get(nodeId); return { node: desc(n, depth == null ? 1 : depth) };
    },
    resolveNode: function (nodeId) {
      const n = get(nodeId); if (!n) return { object: { type: "object", subtype: "null", value: null } };
      return { object: { type: "object", subtype: n.nodeType === 1 ? "node" : undefined, objectId: putObject(n) } };
    },
    requestNode: function (objectId) {
      const reg = ensureObjects();
      const n = reg.map ? reg.map[objectId] : null;
      return { nodeId: n ? idFor(n) : 0 };
    },
    getBoxModel: function (nodeId) {
      const n = get(nodeId); if (!n || !n.getBoundingClientRect) return null;
      const r = n.getBoundingClientRect(); const q = quad(r);
      return { model: { content: q, padding: q, border: q, margin: q, width: r.width, height: r.height } };
    },
    focus: function (nodeId) { const n = get(nodeId); if (n && n.focus) n.focus(); return {}; },
    setAttributeValue: function (nodeId, name, value) { const n = get(nodeId); if (n && n.setAttribute) n.setAttribute(name, value); return {}; },
    removeNode: function (nodeId) { const n = get(nodeId); if (n && n.remove) n.remove(); else if (n && n.parentNode) n.parentNode.removeChild(n); return {}; },
    scrollIntoViewIfNeeded: function (nodeId) { const n = get(nodeId); if (n && n.scrollIntoView) n.scrollIntoView({ block: "center", inline: "center" }); return {}; },
  };
  idFor(document);
})();
