# EasyREG

EasyREG, gerçek log satırlarından açıklanabilir regex ve parser çıktıları üreten,
ürettiği ifadeleri otomatik olarak test eden açık kaynaklı bir araçtır.

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
- Tarayıcı tabanlı log parser çalışma alanı
- JavaScript ve Python parser kodu dışa aktarımı
- Tamamen yerel, bağlam duyarlı alan isimlendirme

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

## Web uygulaması

Log parser çalışma alanını başlatmak için:

```bash
cargo run -p easyreg-server
```

Ardından `http://127.0.0.1:3000` adresini açın. Sunucu; web arayüzünü,
`POST /api/analyze` analiz endpoint'ini ve `GET /api/health` sağlık endpoint'ini
aynı origin üzerinden sunar.

### Yerel semantik alan isimlendirme

Regex üretimi, doğrulama ve capture isimlendirme tamamen yerel Rust motorunda
yapılır. Alan türleri, gözlenen değerler ve `client_ip=`, `status=` veya
`duration=` gibi komşu anahtarlar birlikte değerlendirilerek `level`,
`source_ip`, `http_status` gibi taşınabilir isimler üretilir. Her isimle birlikte
güven seviyesi ve kullanılan kural API yanıtında bulunur. Log verisi hiçbir harici
servise gönderilmez ve API anahtarı gerekmez.

## CLI seçenekleri

| Seçenek | Açıklama |
| --- | --- |
| `-p`, `--positive` | Eşleşmesi gereken örnek. Birden fazla kullanılabilir. |
| `-n`, `--negative` | Eşleşmemesi gereken örnek. Birden fazla kullanılabilir. |
| `--mode full` | Metnin tamamını eşleştirir. Varsayılan moddur. |
| `--mode search` | Metin içinde eşleşen bir bölüm arar. |
| `--compact` | JSON çıktısını tek satır olarak verir. |


## Proje yapısı

| Paket | Görevi |
| --- | --- |
| `easyreg-core` | Temel veri modelleri ve `PatternSpec` |
| `easyreg-detectors` | Alan tür tespiti |
| `easyreg-inference` | Örneklerden yapı çıkarımı |
| `easyreg-semantics` | Yerel ve bağlam duyarlı alan isimlendirme |
| `easyreg-dialects` | Regex dillerine çeviri |
| `easyreg-validation` | Eşleşme ve doğrulama |
| `easyreg-engine` | Analiz akışı, puanlama ve öneri |
| `easyreg-cli` | CLI uygulaması |
| `easyreg-server` | HTTP API ve gömülü log parser web arayüzü |

## Yerel doğrulama

Tek komutla format, Clippy, tüm Rust testleri ve web JavaScript söz dizimi
kontrol edilir:

```bash
./scripts/check.sh
```

`tests/corpus/logs/` altındaki sürümlenmiş corpus dosyaları gerçek log ailelerini,
pozitif varyasyonları, yakın negatifleri, beklenen semantik kuralları ve her satır
için beklenen capture değerlerini tanımlar. Uçtan uca test harness'i üretilen
önerilen regex'i bu corpus'a yeniden uygular; pozitif kapsama veya negatif reddetme
oranı yüzde 100 değilse test başarısız olur. Bu doğrulama tamamen yereldir; CI veya
harici servis gerektirmez.
