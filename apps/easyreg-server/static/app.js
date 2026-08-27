const elements = {
  form: document.querySelector("#analyze-form"),
  positives: document.querySelector("#positive-examples"),
  negatives: document.querySelector("#negative-examples"),
  positiveCount: document.querySelector("#positive-count"),
  negativeCount: document.querySelector("#negative-count"),
  analyzeButton: document.querySelector("#analyze-button"),
  formError: document.querySelector("#form-error"),
  resultStatus: document.querySelector("#result-status"),
  engineStatusText: document.querySelector("#engine-status-text"),
  emptyState: document.querySelector("#empty-state"),
  resultView: document.querySelector("#result-view"),
  candidateTabs: document.querySelector("#candidate-tabs"),
  dialectTabs: document.querySelector("#dialect-tabs"),
  outputTabs: document.querySelector("#output-tabs"),
  score: document.querySelector("#score-metric"),
  coverage: document.querySelector("#coverage-metric"),
  rejection: document.querySelector("#rejection-metric"),
  regex: document.querySelector("#regex-output"),
  copyRegex: document.querySelector("#copy-regex"),
  semanticBadge: document.querySelector("#semantic-badge"),
  semanticMessage: document.querySelector("#semantic-message"),
  semanticFields: document.querySelector("#semantic-fields"),
  validationSummary: document.querySelector("#validation-summary"),
  matchList: document.querySelector("#match-list"),
  noteList: document.querySelector("#note-list"),
};

const labels = {
  strict: "Strict",
  balanced: "Balanced",
  flexible: "Flexible",
  field_classified: "Alan sınıflandırıldı",
  observed_length_range: "Uzunluk aralığı gözlendi",
  flexible_length: "Esnek uzunluk kullanıldı",
  exact_alternation_fallback: "Exact fallback kullanıldı",
  common_affix_fallback: "Ortak başlangıç/bitiş kullanıldı",
  single_example_low_confidence: "Tek örnek — düşük güven",
};

const state = {
  result: null,
  candidateIndex: 0,
  dialect: "java_script",
  output: "regex",
};

function parseLines(value) {
  return value
    .split("\n")
    .map((line) => line.replace(/\r$/, ""))
    .filter((line) => line.length > 0);
}

function updateLineCounts() {
  elements.positiveCount.textContent = `${parseLines(elements.positives.value).length} satır`;
  elements.negativeCount.textContent = `${parseLines(elements.negatives.value).length} satır`;
}

function showError(message) {
  elements.formError.textContent = message;
  elements.formError.hidden = false;
}

function clearError() {
  elements.formError.textContent = "";
  elements.formError.hidden = true;
}

function setLoading(loading) {
  elements.analyzeButton.disabled = loading;
  elements.analyzeButton.querySelector(".button-label").textContent = loading
    ? "Örnekler analiz ediliyor…"
    : "Log parser’ını üret";
  elements.resultStatus.textContent = loading ? "Analiz ediliyor" : "Hazır";
}

function formatPercent(value) {
  return `${Math.round(value * 100)}%`;
}

function selectedCandidate() {
  return state.result?.candidates[state.candidateIndex] ?? null;
}

function renderCandidateTabs() {
  elements.candidateTabs.replaceChildren();

  state.result.candidates.forEach((candidate, index) => {
    const button = document.createElement("button");
    const detail = document.createElement("small");
    const recommended = candidate.strategy === state.result.recommended_strategy;

    button.type = "button";
    button.className = "candidate-tab";
    button.dataset.index = String(index);
    button.setAttribute("aria-selected", String(index === state.candidateIndex));
    button.append(labels[candidate.strategy] ?? candidate.strategy);
    detail.textContent = recommended ? "Önerilen" : `${Math.round(candidate.score * 100)} puan`;
    button.append(detail);
    elements.candidateTabs.append(button);
  });
}

function renderDialectTabs() {
  elements.dialectTabs.hidden = state.output !== "regex";
  elements.dialectTabs.querySelectorAll("button").forEach((button) => {
    button.setAttribute("aria-selected", String(button.dataset.dialect === state.dialect));
  });
}

function renderOutputTabs() {
  elements.outputTabs.querySelectorAll("button").forEach((button) => {
    button.setAttribute("aria-selected", String(button.dataset.output === state.output));
  });
}

function javascriptParser(pattern) {
  return `const LOG_PATTERN = new RegExp(${JSON.stringify(pattern)});\n\nfunction parseLog(line) {\n  const match = LOG_PATTERN.exec(line);\n  return match ? { ...match.groups } : null;\n}`;
}

function pythonParser(pattern) {
  return `import re\n\nLOG_PATTERN = re.compile(${JSON.stringify(pattern)})\n\ndef parse_log(line: str) -> dict[str, str] | None:\n    match = LOG_PATTERN.fullmatch(line)\n    return match.groupdict() if match else None`;
}

function outputText(candidate) {
  if (state.output === "javascript_parser") {
    return javascriptParser(candidate.renderings.java_script);
  }
  if (state.output === "python_parser") {
    return pythonParser(candidate.renderings.python);
  }
  return candidate.renderings[state.dialect] ?? "Bu dil için çıktı yok.";
}

function renderSemantics() {
  const semantics = state.result.semantics;
  elements.semanticFields.replaceChildren();

  if (semantics.status === "complete") {
    elements.semanticBadge.textContent = "Yerel kural motoru";
    elements.semanticMessage.textContent = "Alan adları veri tipi, gözlenen değerler ve komşu anahtarlar kullanılarak tamamen yerel üretildi.";
    semantics.fields.forEach((field) => {
      const item = document.createElement("article");
      const header = document.createElement("div");
      const name = document.createElement("code");
      const confidence = document.createElement("span");
      const label = document.createElement("strong");
      const description = document.createElement("span");
      item.className = "semantic-field";
      header.className = "semantic-field-header";
      name.textContent = field.name;
      confidence.className = "confidence";
      confidence.textContent = field.confidence;
      label.textContent = field.label;
      description.textContent = field.description;
      header.append(name, confidence);
      item.append(header, label, description);
      elements.semanticFields.append(item);
    });
    return;
  }

  const messages = {
    not_applicable: "Bu adayda isimlendirilecek capture alanı bulunamadı.",
  };
  elements.semanticBadge.textContent = "Yerel kural motoru";
  elements.semanticMessage.textContent = messages[semantics.status] ?? "Semantik bilgi bulunamadı.";
}

function appendMatch(result, expectedMatch) {
  const item = document.createElement("div");
  const line = document.createElement("div");
  const icon = document.createElement("span");
  const value = document.createElement("span");
  const passed = result.matched === expectedMatch;

  item.className = "match-item";
  line.className = "match-line";
  icon.className = `match-icon${passed ? "" : " failed"}`;
  icon.textContent = passed ? "✓" : "!";
  icon.setAttribute("aria-label", passed ? "Beklenen sonuç" : "Beklenmeyen sonuç");
  value.className = "match-value";
  value.textContent = result.input;
  value.title = result.input;
  line.append(icon, value);
  item.append(line);

  const captures = Object.entries(result.captures ?? {});
  if (captures.length > 0) {
    const captureList = document.createElement("div");
    captureList.className = "capture-list";
    captures.forEach(([name, capture]) => {
      const chip = document.createElement("span");
      chip.className = "capture-chip";
      chip.textContent = `${name}: ${capture}`;
      captureList.append(chip);
    });
    item.append(captureList);
  }

  elements.matchList.append(item);
}

function describeNote(note) {
  const attributes = Object.entries(note.attributes ?? {})
    .map(([key, value]) => `${key}: ${value}`)
    .join(" · ");
  const field = note.field_id ? `${note.field_id} · ` : "";
  return `${field}${attributes || "Yapısal çıkarım bilgisi"}`;
}

function renderCandidate() {
  const candidate = selectedCandidate();
  if (!candidate) return;

  elements.score.textContent = `${(candidate.score * 100).toFixed(1)}`;
  elements.coverage.textContent = formatPercent(candidate.validation.positive_coverage);
  elements.rejection.textContent = formatPercent(candidate.validation.negative_rejection);
  elements.regex.textContent = outputText(candidate);

  renderCandidateTabs();
  renderDialectTabs();
  renderOutputTabs();
  renderSemantics();

  elements.matchList.replaceChildren();
  candidate.validation.positive_results.forEach((result) => appendMatch(result, true));
  candidate.validation.negative_results.forEach((result) => appendMatch(result, false));

  const passed = [
    ...candidate.validation.positive_results.map((result) => result.matched),
    ...candidate.validation.negative_results.map((result) => !result.matched),
  ].filter(Boolean).length;
  const total =
    candidate.validation.positive_results.length + candidate.validation.negative_results.length;
  elements.validationSummary.textContent = `${passed}/${total} beklenen sonuç`;

  elements.noteList.replaceChildren();
  candidate.notes.forEach((note) => {
    const item = document.createElement("div");
    const title = document.createElement("strong");
    const detail = document.createElement("span");
    item.className = "note-item";
    title.textContent = labels[note.code] ?? note.code;
    detail.textContent = describeNote(note);
    item.append(title, detail);
    elements.noteList.append(item);
  });

  if (candidate.notes.length === 0) {
    const item = document.createElement("div");
    item.className = "note-item";
    item.textContent = "Bu aday için ek çıkarım notu yok.";
    elements.noteList.append(item);
  }
}

function renderResult(result) {
  state.result = result;
  const recommendedIndex = result.candidates.findIndex(
    (candidate) => candidate.strategy === result.recommended_strategy,
  );
  state.candidateIndex = recommendedIndex >= 0 ? recommendedIndex : 0;
  state.dialect = "java_script";
  state.output = "regex";

  elements.emptyState.hidden = true;
  elements.resultView.hidden = false;
  elements.resultStatus.textContent = `${result.candidates.length} aday hazır`;
  renderCandidate();
}

async function submitAnalysis(event) {
  event.preventDefault();
  clearError();

  const positiveExamples = parseLines(elements.positives.value);
  if (positiveExamples.length === 0) {
    showError("En az bir pozitif örnek girmelisiniz.");
    elements.positives.focus();
    return;
  }

  const selectedMode = elements.form.querySelector('input[name="match_mode"]:checked');
  const request = {
    positive_examples: positiveExamples,
    negative_examples: parseLines(elements.negatives.value),
    match_mode: selectedMode?.value ?? "full",
  };

  setLoading(true);
  try {
    const response = await fetch("/api/analyze", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) {
      throw new Error(payload?.error?.message ?? "Analiz isteği tamamlanamadı.");
    }
    renderResult(payload);
  } catch (error) {
    showError(error instanceof Error ? error.message : "Beklenmeyen bir hata oluştu.");
    elements.resultStatus.textContent = "Hata";
  } finally {
    setLoading(false);
    if (state.result) {
      elements.resultStatus.textContent = `${state.result.candidates.length} aday hazır`;
    }
  }
}

elements.form.addEventListener("submit", submitAnalysis);
elements.positives.addEventListener("input", updateLineCounts);
elements.negatives.addEventListener("input", updateLineCounts);

elements.candidateTabs.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-index]");
  if (!button) return;
  state.candidateIndex = Number(button.dataset.index);
  renderCandidate();
});

elements.dialectTabs.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-dialect]");
  if (!button) return;
  state.dialect = button.dataset.dialect;
  renderCandidate();
});

elements.outputTabs.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-output]");
  if (!button) return;
  state.output = button.dataset.output;
  renderCandidate();
});

elements.copyRegex.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(elements.regex.textContent);
    elements.copyRegex.textContent = "Kopyalandı";
    window.setTimeout(() => {
      elements.copyRegex.textContent = "Kopyala";
    }, 1400);
  } catch {
    showError("Regex panoya kopyalanamadı; metni elle seçebilirsiniz.");
  }
});

updateLineCounts();

fetch("/api/health")
  .then((response) => response.json())
  .then((health) => {
    elements.engineStatusText.textContent = health.external_services
      ? "Motor + harici servis"
      : "Tamamen yerel motor";
  })
  .catch(() => {
    elements.engineStatusText.textContent = "Motor durumu alınamadı";
  });
