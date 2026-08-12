# EasyREG

EasyREG, örnek metinlerden düzenli ifade (regex) üreten ve ürettiği ifadeleri
otomatik olarak test eden açık kaynaklı bir araçtır.

Eşleşmesi gereken örnekleri girersiniz. İsterseniz eşleşmemesi gereken örnekleri
de eklersiniz. EasyREG örneklerin ortak yapısını analiz eder, farklı esneklik
seviyelerinde regex adayları oluşturur ve en uygun adayı önerir.

## Özellikler

- Pozitif ve negatif örneklerle regex üretimi
- Sabit metinlerin ve değişken alanların otomatik ayrılması
- IPv4, IPv6, UUID, e-posta, URL, ISO tarih, sayı ve hexadecimal değer tespiti
- `strict`, `balanced` ve `flexible` olmak üzere üç farklı aday
- JavaScript, Python ve PCRE2 regex çıktısı
- Tam metin eşleştirme ve metin içinde arama modları
- Named capture desteği
- Her örnek için eşleşme sonucu ve yakalanan alanlar
- Eşleşme başarısı, negatif örnekleri reddetme oranı ve aday puanlama
- Makine tarafından işlenebilir JSON çıktı

## Nasıl çalışır?

```text
Örnekler
   ↓
Yapı ve alan türü çıkarımı
   ↓
PatternSpec
   ↓
Regex üretimi
   ↓
Doğrulama ve aday seçimi
```

EasyREG önce regex motorundan bağımsız bir `PatternSpec` oluşturur. Daha sonra
bu yapı hedef regex diline çevrilir. Böylece aynı analiz sonucu farklı regex
motorları için tekrar kullanılabilir.

## Hızlı başlangıç

Depoda kullanılacak Rust sürümü `rust-toolchain.toml` dosyasında tanımlıdır.

```powershell
cargo run -p easyreg-cli -- infer `
  -p "INV-2026-00127" `
  -p "INV-2025-84621" `
  -p "INV-2026-18342" `
  -n "ORD-2026-00127" `
  -n "INV-26-127" `
  -n "INV-2026-ABCDE"
```

Bu örnek için önerilen JavaScript çıktısı:

```regex
^(?:INV-(?<field_1>[0-9]{4})-(?<field_2>[0-9]{5}))$
```

## CLI seçenekleri

| Seçenek | Açıklama |
| --- | --- |
| `-p`, `--positive` | Eşleşmesi gereken örnek. Birden fazla kullanılabilir. |
| `-n`, `--negative` | Eşleşmemesi gereken örnek. Birden fazla kullanılabilir. |
| `--mode full` | Metnin tamamını eşleştirir. Varsayılan moddur. |
| `--mode search` | Metin içinde eşleşen bir bölüm arar. |
| `--compact` | JSON çıktısını tek satır olarak verir. |

Komut yardımını görüntülemek için:

```powershell
cargo run -p easyreg-cli -- infer --help
```

## Proje yapısı

| Paket | Görevi |
| --- | --- |
| `easyreg-core` | Temel veri modelleri ve `PatternSpec` |
| `easyreg-detectors` | Alan türü tespiti |
| `easyreg-inference` | Örneklerden yapı çıkarımı |
| `easyreg-dialects` | Regex dillerine çeviri |
| `easyreg-validation` | Eşleşme ve doğrulama |
| `easyreg-engine` | Analiz akışı, puanlama ve öneri |
| `easyreg-cli` | Komut satırı uygulaması |

## Geliştirme

Tüm testleri çalıştırmak için:

```powershell
cargo test --workspace
```

Kod kalitesi kontrolü için:

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

## Yol haritası

- Dosyadan toplu örnek okuma ve benzer formatları kümeleme
- Web arayüzü ve HTTP API
- Daha fazla alan türü ve regex dili
- Alanları arayüzden adlandırma ve düzenleme
- Farklı araçlar için dışa aktarma seçenekleri

## Durum

Proje aktif geliştirme aşamasındadır. Mevcut sürüm, örneklerden regex üretme
motorunu ve komut satırı arayüzünü içerir.
