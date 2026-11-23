const verseToken = /#[^\n]*|"(?:\\.|[^"\\\n])*"|<(?:decides|transacts|suspends|computes|converges|no_rollback|varies)>|\b(?:array|block|branch|break|case|class|defer|else|enum|false|float|for|if|int|logic|loop|map|not|option|race|return|rush|set|spawn|string|struct|sync|true|using|var|void)\b|\b\d+(?:\.\d+)?\b/g;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function drawSyntax(source, highlighted) {
    const fragment = document.createDocumentFragment();
    let end = 0;
    for (const match of source.matchAll(verseToken)) {
        const token = match[0];
        const span = document.createElement("span");
        if (token.startsWith("#")) {
            span.className = "comment";
        } else if (token.startsWith("<")) {
            span.className = "specifier";
        } else if (/^["\d]/.test(token)) {
            span.className = "literal";
        } else {
            span.className = "keyword";
        }
        span.textContent = token;
        fragment.append(source.slice(end, match.index), span);
        end = match.index + token.length;
    }
    fragment.append(source.slice(end), "\n");
    highlighted.replaceChildren(fragment);
}

function sourceRange(source, start, end) {
    const bytes = encoder.encode(source);
    if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || end < start || end > bytes.length) {
        return null;
    }
    // The checker uses UTF-8 byte offsets. The browser selection uses UTF-16.
    const before = decoder.decode(bytes.subarray(0, start));
    const lines = before.split("\n");
    return {
        start: before.length,
        end: decoder.decode(bytes.subarray(0, end)).length,
        line: lines.length,
        column: [...lines.at(-1)].length + 1,
    };
}

export function createEditor() {
    const panel = document.querySelector("#verse-editor");
    const stationButtons = [...panel.querySelectorAll("[data-station]")];
    const source = panel.querySelector("#verse-source");
    const title = panel.querySelector("#source-label");
    const highlighted = panel.querySelector("#verse-syntax");
    const lines = panel.querySelector("#line-numbers");
    const applyButton = panel.querySelector("#apply-draft");
    const resetButton = panel.querySelector("#reset-draft");
    const status = panel.querySelector("#editor-status");
    const diagnostics = panel.querySelector("#editor-diagnostics");
    const drafts = new Map(stationButtons.map(button => [button.dataset.station, {
        name: button.textContent,
        original: null,
        source: "",
        diagnostics: [],
        status: "Loading station source...",
        tone: "",
    }]));

    let selected = "beacon";
    let submit = null;
    let unavailable = false;
    let pending = null;
    let requestId = 0;

    function syncScroll() {
        lines.scrollTop = source.scrollTop;
        highlighted.scrollTop = source.scrollTop;
        highlighted.scrollLeft = source.scrollLeft;
    }

    function renderSource() {
        const lineCount = source.value.split("\n").length;
        lines.textContent = Array.from({ length: lineCount }, (_, index) => index + 1).join("\n");
        drawSyntax(source.value, highlighted);
        syncScroll();
    }

    function reveal(range) {
        source.focus({ preventScroll: true });
        source.setSelectionRange(range.start, range.end);
        const lineHeight = parseFloat(getComputedStyle(source).lineHeight);
        source.scrollTop = Math.max(0, (range.line - 2) * lineHeight);
        syncScroll();
    }

    function renderDiagnostic(diagnostic) {
        const item = document.createElement("li");
        item.dataset.severity = diagnostic.severity;
        const code = document.createElement(diagnostic.range ? "button" : "span");
        code.className = "diagnostic-code";
        code.textContent = diagnostic.code;
        if (diagnostic.range) {
            const { line, column } = diagnostic.range;
            code.type = "button";
            code.textContent += " · " + line + ":" + column;
            code.title = "Select line " + line + ", column " + column;
            code.addEventListener("click", () => reveal(diagnostic.range));
        }
        const message = document.createElement("p");
        message.textContent = diagnostic.message;
        item.append(code, message);
        if (diagnostic.help) {
            const help = document.createElement("p");
            help.className = "help";
            help.textContent = diagnostic.help;
            item.append(help);
        }
        return item;
    }

    function refreshPanel() {
        const draft = drafts.get(selected);
        const busy = pending?.station === selected;
        source.readOnly = draft.original === null || busy;
        applyButton.disabled = !submit || draft.original === null || pending !== null;
        resetButton.disabled = source.readOnly || draft.source === draft.original;
        if (draft.diagnostics.some(item => item.severity === "error")) {
            source.setAttribute("aria-invalid", "true");
        } else {
            source.removeAttribute("aria-invalid");
        }
        status.dataset.tone = unavailable ? "error" : draft.tone;
        const message = unavailable ? "The game must start before you can apply a draft." : draft.status;
        if (status.textContent !== message) {
            status.textContent = message;
        }
        diagnostics.replaceChildren(...draft.diagnostics.map(renderDiagnostic));
    }

    function selectStation(station) {
        selected = station;
        title.textContent = station + ".verse";
        for (const button of stationButtons) {
            button.setAttribute("aria-pressed", String(button.dataset.station === station));
        }
        source.value = drafts.get(station).source;
        source.scrollTop = 0;
        source.scrollLeft = 0;
        renderSource();
        refreshPanel();
    }

    function recordInput() {
        const draft = drafts.get(selected);
        draft.source = source.value;
        draft.diagnostics = [];
        draft.status = "Draft changed. Apply to update the station.";
        draft.tone = "";
        renderSource();
        refreshPanel();
    }

    function apply() {
        if (applyButton.disabled) {
            return;
        }
        const draft = drafts.get(selected);
        const id = ++requestId;
        pending = { id, station: selected, source: draft.source };
        draft.diagnostics = [];
        draft.status = "Checking " + selected + ".verse...";
        draft.tone = "";
        refreshPanel();
        try {
            if (submit(id, selected, draft.source)) {
                return;
            }
            draft.status = "Another edit is in progress. Try applying again.";
        } catch (error) {
            console.error(error);
            draft.status = "Could not submit the draft.";
        }
        pending = null;
        draft.tone = "error";
        refreshPanel();
    }

    globalThis.observatoryEditorDiagnostic = (id, code, severity, message, help, start, end) => {
        if (pending?.id !== id) {
            return;
        }
        const draft = drafts.get(pending.station);
        const range = sourceRange(pending.source, start, end);
        draft.diagnostics.push({ code, severity: String(severity).toLowerCase(), message, help, range });
        if (pending.station === selected) {
            refreshPanel();
        }
    };

    globalThis.observatoryEditorFinished = (id, applied, message) => {
        if (pending?.id !== id) {
            return;
        }
        const draft = drafts.get(pending.station);
        draft.status = message || (applied
            ? draft.name + " updated."
            : "Draft rejected. The current station is still running.");
        draft.tone = applied ? "good" : "error";
        pending = null;
        refreshPanel();
    };

    for (const button of stationButtons) {
        button.addEventListener("click", () => selectStation(button.dataset.station));
    }
    applyButton.addEventListener("click", apply);
    resetButton.addEventListener("click", () => {
        const draft = drafts.get(selected);
        source.value = draft.original;
        recordInput();
        draft.status = "Bundled draft restored. Apply to update the station.";
        refreshPanel();
    });
    source.addEventListener("input", recordInput);
    source.addEventListener("scroll", syncScroll);
    panel.addEventListener("keydown", event => {
        event.stopPropagation();
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            apply();
        }
        if (event.target === source && event.key === "Tab" && !event.shiftKey
            && !event.altKey && !event.metaKey && !event.ctrlKey && !source.readOnly) {
            event.preventDefault();
            const { selectionStart: start, selectionEnd: end } = source;
            if (start === end) {
                source.setRangeText("    ", start, end, "end");
            } else {
                const lineStart = start === 0 ? 0 : source.value.lastIndexOf("\n", start - 1) + 1;
                const indented = "    " + source.value.slice(lineStart, end).replace(/\n(?!$)/g, "\n    ");
                source.setRangeText(indented, lineStart, end, "select");
            }
            recordInput();
        }
    });
    panel.addEventListener("keyup", event => event.stopPropagation());

    for (const [station, draft] of drafts) {
        fetch("./assets/" + station + ".verse").then(response => {
            if (!response.ok) {
                throw new Error("HTTP " + response.status);
            }
            return response.text();
        }).then(text => {
            draft.original = text.replace(/\r\n?/g, "\n");
            draft.source = draft.original;
            draft.status = "Edit " + station + ".verse and apply the draft.";
            if (selected === station) {
                selectStation(station);
            }
        }).catch(error => {
            console.error(error);
            draft.status = "Could not load " + station + ".verse.";
            draft.tone = "error";
            if (selected === station) {
                refreshPanel();
            }
        });
    }
    refreshPanel();

    return {
        connect(submitDraft) {
            submit = submitDraft;
            refreshPanel();
        },
        unavailable() {
            unavailable = true;
            submit = null;
            refreshPanel();
        },
    };
}
