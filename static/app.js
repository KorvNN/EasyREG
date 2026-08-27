const elements = {
  form: document.querySelector("#analyze-form"),
  positives: document.querySelector("#positive-examples"),
  negatives: document.querySelector("#negative-examples"),
  positiveCount: document.querySelector("#positive-count"),
  negativeCount: document.querySelector("#negative-count"),
  logFile: document.querySelector("#log-file"),
  uploadBox: document.querySelector("#upload-box"),
  uploadStatus: document.querySelector("#upload-status"),
  analyzeButton: document.querySelector("#analyze-button"),
  formError: document.querySelector("#form-error"),
  emptyState: document.querySelector("#empty-state"),
  resultView: document.querySelector("#result-view"),
  candidateTabs: document.querySelector("#candidate-tabs"),
  dialectTabs: document.querySelector("#dialect-tabs"),
  outputTabs: document.querySelector("#output-tabs"),
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

const MAX_IMPORTED_LINES = 240;
const MAX_SAMPLES_PER_SHAPE = 8;
const MAX_IMPORTED_LINE_BYTES = 32 * 1024;
const MAX_IMPORTED_SAMPLE_BYTES = 768 * 1024;
const MAX_REQUEST_BYTES = 1536 * 1024;

const strategyLabels = {
  strict: "Kesin",
  balanced: "Dengeli",
  flexible: "Esnek",
};

const strategyDescriptions = {
  strict: "Yalnızca gözlenen biçime en yakın desen",
  balanced: "Gözlenen farkları kontrollü biçimde genelleştirir",
  flexible: "Yeni değer aralıklarına daha açık desen",
};

const noteTitles = {
  field_classified: "Alan türü belirlendi",
  observed_length_range: "Uzunluk sınırı belirlendi",
  flexible_length: "Değişken uzunluk kullanıldı",
  exact_alternation_fallback: "Desen örneklerle sınırlandı",
  common_affix_fallback: "Ortak bölümler korundu",
  single_example_low_confidence: "Daha fazla satır gerekli",
};

const confidenceLabels = {
  high: "Yüksek güven",
  medium: "Orta güven",
  low: "Düşük güven",
};

const kindLabels = {
  ipv4: "IPv4 adresi",
  ipv6: "IPv6 adresi",
  uuid: "UUID",
  email: "E-posta adresi",
  url: "URL",
  path: "Yol",
  date_iso: "ISO tarihi",
  time: "Saat",
  integer: "Tam sayı",
  decimal: "Ondalık sayı",
  hexadecimal: "Onaltılık değer",
  uppercase: "Büyük harfli metin",
  lowercase: "Küçük harfli metin",
  alphabetic: "Harflerden oluşan metin",
  alphanumeric: "Harf ve rakamlardan oluşan metin",
  whitespace: "Boşluk",
  non_whitespace: "Boşluksuz metin",
  text: "Metin",
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
    .filter((line) => line.trim().length > 0);
}

function updateLineCounts() {
  elements.positiveCount.textContent = `${parseLines(elements.positives.value).length} satır`;
  elements.negativeCount.textContent = `${parseLines(elements.negatives.value).length} satır`;
}

function setUploadStatus(message) {
  elements.uploadStatus.textContent = message;
  elements.uploadStatus.hidden = false;
}

function lineShape(line) {
  return line
    .replace(/\b[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}\b/gi, "<uuid>")
    .replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, "<ipv4>")
    .replace(/\b\d{4}-\d{2}-\d{2}\b/g, "<date>")
    .replace(/\b\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?\b/g, "<time>")
    .replace(/\b0x[0-9a-f]+\b/gi, "<hex>")
    .replace(/\b\d+(?:\.\d+)?\b/g, "<number>")
    .replace(/"[^"\r\n]*"/g, '"<text>"')
    .replace(/'[^'\r\n]*'/g, "'<text>'")
    .replace(/\b[A-Za-z0-9_-]{24,}\b/g, "<token>");
}

async function sampleLogFile(file) {
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const encoder = new TextEncoder();
  const reader = file.stream().getReader();
  const samplesByShape = new Map();
  let buffer = "";
  let discardingLongLine = false;
  let sampledBytes = 0;
  let sampledLines = 0;
  let scannedLines = 0;
  let skippedLongLines = 0;

  function addSample(rawLine) {
    const line = rawLine.replace(/\r$/, "");
    if (line.trim().length === 0) return;
    scannedLines += 1;

    const lineBytes = encoder.encode(line).byteLength;
    if (lineBytes > MAX_IMPORTED_LINE_BYTES) {
      skippedLongLines += 1;
      return;
    }

    const shape = lineShape(line);
    const existingBucket = samplesByShape.get(shape);
    if (existingBucket?.length >= MAX_SAMPLES_PER_SHAPE) return;

    if (
      sampledLines < MAX_IMPORTED_LINES
      && sampledBytes + lineBytes <= MAX_IMPORTED_SAMPLE_BYTES
    ) {
      const bucket = existingBucket ?? [];
      bucket.push({ line, bytes: lineBytes });
      samplesByShape.set(shape, bucket);
      sampledLines += 1;
      sampledBytes += lineBytes;
      return;
    }

    if (existingBucket) return;

    const donor = [...samplesByShape.entries()]
      .filter(([, bucket]) => bucket.length > 1)
      .sort((left, right) => right[1].length - left[1].length)[0];
    if (!donor) return;

    const removed = donor[1].at(-1);
    if (!removed || sampledBytes - removed.bytes + lineBytes > MAX_IMPORTED_SAMPLE_BYTES) return;
    donor[1].pop();
    samplesByShape.set(shape, [{ line, bytes: lineBytes }]);
    sampledBytes += lineBytes - removed.bytes;
  }

  function consumeText(text) {
    buffer += text;
    let newlineIndex = buffer.indexOf("\n");
    while (newlineIndex >= 0) {
      const line = buffer.slice(0, newlineIndex);
      buffer = buffer.slice(newlineIndex + 1);
      if (discardingLongLine) {
        discardingLongLine = false;
      } else {
        addSample(line);
      }
      newlineIndex = buffer.indexOf("\n");
    }

    if (encoder.encode(buffer).byteLength > MAX_IMPORTED_LINE_BYTES) {
      buffer = "";
      discardingLongLine = true;
      scannedLines += 1;
      skippedLongLines += 1;
    }
  }

  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      if (value.includes(0)) {
        throw new Error("İkili dosyalar analiz edilemez. Metin biçiminde bir log dosyası seçin.");
      }
      consumeText(decoder.decode(value, { stream: true }));
    }
    consumeText(decoder.decode());
    if (!discardingLongLine && buffer.length > 0) addSample(buffer);
  } catch (error) {
    if (error instanceof TypeError) {
      throw new Error("Dosya UTF-8 olarak okunamadı.");
    }
    throw error;
  } finally {
    reader.releaseLock();
  }

  return {
    lines: [...samplesByShape.values()].flat().map((sample) => sample.line),
    scannedLines,
    skippedLongLines,
  };
}

async function addLogFile(file) {
  clearError();
  if (!file) return;
  elements.uploadStatus.hidden = true;
  elements.uploadStatus.textContent = "";
  elements.logFile.value = "";
  if (file.size === 0) {
    showError("Seçilen dosya boş.");
    return;
  }

  elements.uploadBox.classList.add("is-loading");
  try {
    const { lines, scannedLines, skippedLongLines } = await sampleLogFile(file);
    if (lines.length === 0) {
      throw new Error("Dosyada analiz edilecek bir log satırı bulunamadı.");
    }

    const existing = elements.positives.value.replace(/\s+$/, "");
    elements.positives.value = existing
      ? `${existing}\n${lines.join("\n")}`
      : lines.join("\n");
    updateLineCounts();
    const wasSampled = scannedLines > lines.length;
    const status = wasSampled
      ? `${file.name} · ${scannedLines.toLocaleString("tr-TR")} satır tarandı · ${lines.length} temsilî satır eklendi`
      : `${file.name} · ${lines.length} satır eklendi`;
    const skipped = skippedLongLines > 0
      ? ` · ${skippedLongLines.toLocaleString("tr-TR")} çok uzun satır atlandı`
      : "";
    setUploadStatus(`${status}${skipped}`);
  } catch (error) {
    showError(error instanceof Error ? error.message : "Dosya okunamadı.");
  } finally {
    elements.uploadBox.classList.remove("is-loading");
  }
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
    ? "Loglar analiz ediliyor…"
    : "Regex ve parser üret";
}

function formatPercent(value) {
  return `${Math.round(value * 100)}%`;
}

function selectedCandidate() {
  return state.result?.candidates[state.candidateIndex] ?? null;
}

function validationCount(candidate) {
  const positivePassed = candidate.validation.positive_results.filter(
    (result) => result.matched,
  ).length;
  const negativePassed = candidate.validation.negative_results.filter(
    (result) => !result.matched,
  ).length;
  return {
    passed: positivePassed + negativePassed,
    total:
      candidate.validation.positive_results.length + candidate.validation.negative_results.length,
  };
}

function formatValidation(results, expectedMatch) {
  if (results.length === 0) return "Eklenmedi";
  const passed = results.filter((result) => result.matched === expectedMatch).length;
  return `${passed}/${results.length} · ${formatPercent(passed / results.length)}`;
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
    const validation = validationCount(candidate);
    button.append(strategyLabels[candidate.strategy] ?? "Alternatif");
    button.title = strategyDescriptions[candidate.strategy] ?? "";
    detail.textContent = recommended ? "Önerilen" : `${validation.passed}/${validation.total} doğru`;
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

function pythonParser(pattern, matchMode) {
  const method = matchMode === "search" ? "search" : "fullmatch";
  return `import re\n\nLOG_PATTERN = re.compile(${JSON.stringify(pattern)})\n\ndef parse_log(line: str) -> dict[str, str] | None:\n    match = LOG_PATTERN.${method}(line)\n    return match.groupdict() if match else None`;
}

function outputText(candidate) {
  if (state.output === "javascript_parser") {
    return javascriptParser(candidate.renderings.java_script);
  }
  if (state.output === "python_parser") {
    return pythonParser(candidate.renderings.python, candidate.spec.match_mode);
  }
  return candidate.renderings[state.dialect] ?? "Seçilen biçim için çıktı üretilemedi.";
}

function renderSemantics() {
  const semantics = state.result.semantics;
  elements.semanticFields.replaceChildren();

  if (semantics.status === "complete") {
    elements.semanticBadge.textContent = "Yerel";
    elements.semanticMessage.textContent = `${semantics.fields.length} değişken alan bulundu ve log bağlamına göre adlandırıldı.`;
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
      confidence.textContent = confidenceLabels[field.confidence] ?? "Güven belirtilmedi";
      label.textContent = field.label;
      description.textContent = field.description;
      header.append(name, confidence);
      item.append(header, label, description);
      elements.semanticFields.append(item);
    });
    return;
  }

  const messages = {
    not_applicable: "Bu desende adlandırılacak değişken alan bulunamadı.",
  };
  elements.semanticBadge.textContent = "Yerel";
  elements.semanticMessage.textContent = messages[semantics.status] ?? "Alan bilgisi üretilemedi.";
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

function displayFieldName(fieldId) {
  const semantic = state.result?.semantics?.fields.find((field) => field.field_id === fieldId);
  if (semantic) return semantic.name;
  const number = fieldId?.match(/\d+$/)?.[0];
  return number ? `Alan ${number}` : "Değişken alan";
}

function describeNote(note) {
  const field = displayFieldName(note.field_id);
  const minimum = note.attributes?.min;
  const maximum = note.attributes?.max;

  switch (note.code) {
    case "field_classified":
      return `${field}: ${kindLabels[note.attributes?.kind] ?? "Değişken değer"}`;
    case "observed_length_range":
      return minimum === maximum
        ? `${field}: ${minimum} karakter`
        : `${field}: ${minimum}–${maximum} karakter`;
    case "flexible_length":
      return `${field}: en az ${minimum} karakter, üst sınır yok`;
    case "exact_alternation_fallback":
      return "Eşleşmemesi gereken satırları reddetmek için yalnızca verilen satırlar kabul edilir.";
    case "common_affix_fallback":
      return "Satırların ortak başlangıç ve bitiş bölümleri korunur; orta bölüm değişken kabul edilir.";
    case "single_example_low_confidence":
      return "Tek satırdan çıkarım yapıldı. Deseni başka gerçek satırlarla doğrulayın.";
    default:
      return "Desenin oluşturulmasında ek bir sınır uygulandı.";
  }
}

function renderCandidate() {
  const candidate = selectedCandidate();
  if (!candidate) return;

  elements.coverage.textContent = formatValidation(candidate.validation.positive_results, true);
  elements.rejection.textContent = formatValidation(candidate.validation.negative_results, false);
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
  elements.validationSummary.textContent = `${passed}/${total} kontrol geçti`;

  elements.noteList.replaceChildren();
  candidate.notes.forEach((note) => {
    const item = document.createElement("div");
    const title = document.createElement("strong");
    const detail = document.createElement("span");
    item.className = "note-item";
    title.textContent = noteTitles[note.code] ?? "Desen sınırı";
    detail.textContent = describeNote(note);
    item.append(title, detail);
    elements.noteList.append(item);
  });

  if (candidate.notes.length === 0) {
    const item = document.createElement("div");
    item.className = "note-item";
    item.textContent = "Bu seçenek için ek bir desen kararı yok.";
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
  renderCandidate();
}

async function submitAnalysis(event) {
  event.preventDefault();
  clearError();

  const positiveExamples = parseLines(elements.positives.value);
  if (positiveExamples.length === 0) {
    showError("En az bir eşleşmesi gereken log satırı girin.");
    elements.positives.focus();
    return;
  }

  const selectedMode = elements.form.querySelector('input[name="match_mode"]:checked');
  const request = {
    positive_examples: positiveExamples,
    negative_examples: parseLines(elements.negatives.value),
    match_mode: selectedMode?.value ?? "full",
  };
  const requestBody = JSON.stringify(request);
  if (new TextEncoder().encode(requestBody).byteLength > MAX_REQUEST_BYTES) {
    showError("Girdi analiz sınırını aşıyor. Daha küçük ve temsilî bir log kümesi kullanın.");
    return;
  }

  setLoading(true);
  try {
    const response = await fetch("/api/analyze", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: requestBody,
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) {
      const message = response.status === 400
        ? "Girdi analiz edilemedi. Satırların boş olmadığını ve aynı biçime ait olduğunu denetleyin."
        : "Analiz tamamlanamadı. Yerel motoru yeniden deneyin.";
      throw new Error(message);
    }
    renderResult(payload);
  } catch (error) {
    showError(error instanceof Error ? error.message : "Beklenmeyen bir hata oluştu.");
  } finally {
    setLoading(false);
  }
}

elements.form.addEventListener("submit", submitAnalysis);
elements.logFile.addEventListener("change", () => addLogFile(elements.logFile.files?.[0]));
elements.positives.addEventListener("input", updateLineCounts);
elements.negatives.addEventListener("input", updateLineCounts);

for (const eventName of ["dragenter", "dragover"]) {
  elements.uploadBox.addEventListener(eventName, (event) => {
    event.preventDefault();
    elements.uploadBox.classList.add("is-dragging");
  });
}

for (const eventName of ["dragleave", "drop"]) {
  elements.uploadBox.addEventListener(eventName, (event) => {
    event.preventDefault();
    elements.uploadBox.classList.remove("is-dragging");
  });
}

elements.uploadBox.addEventListener("drop", (event) => addLogFile(event.dataTransfer?.files?.[0]));

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
      elements.copyRegex.textContent = "Çıktıyı kopyala";
    }, 1400);
  } catch {
    showError("Çıktı panoya kopyalanamadı. Metni elle seçebilirsiniz.");
  }
});

updateLineCounts();
