import time
import zlib
import lz4.frame
import zstandard as zstd
import os
import sys

# Добавляем путь к локальной библиотеке
sys.path.insert(0, os.path.join(os.getcwd(), "python"))

try:
    import logzip
    from logzip import compress, decompress
except ImportError:
    print("Ошибка: logzip не установлен или не найден в python/")
    sys.exit(1)

def benchmark():
    log_path = "log.log"
    if not os.path.exists(log_path):
        print(f"Файл {log_path} не найден. Создаю тестовый лог...")
        with open(log_path, "w") as f:
            for i in range(10000):
                f.write(f"2024-04-22 12:00:0{i%10} [INFO] User {i%100} logged in from 192.168.1.{i%255}\n")
    
    with open(log_path, "rb") as f:
        raw_data = f.read()
    
    raw_text = raw_data.decode("utf-8", errors="ignore")
    orig_size = len(raw_data)
    
    results = []

    # 1. zlib
    start = time.perf_counter()
    zlib_comp = zlib.compress(raw_data, level=6)
    zlib_time = time.perf_counter() - start
    results.append({
        "Method": "zlib (lvl 6)",
        "Size (bytes)": len(zlib_comp),
        "Ratio": f"{len(zlib_comp)/orig_size:.2%}",
        "Comp Time (ms)": zlib_time * 1000,
        "Type": "binary"
    })

    # 2. lz4
    start = time.perf_counter()
    lz4_comp = lz4.frame.compress(raw_data)
    lz4_time = time.perf_counter() - start
    results.append({
        "Method": "lz4",
        "Size (bytes)": len(lz4_comp),
        "Ratio": f"{len(lz4_comp)/orig_size:.2%}",
        "Comp Time (ms)": lz4_time * 1000,
        "Type": "binary"
    })

    # 3. zstd
    cctx = zstd.ZstdCompressor(level=3)
    start = time.perf_counter()
    zstd_comp = cctx.compress(raw_data)
    zstd_time = time.perf_counter() - start
    results.append({
        "Method": "zstd (lvl 3)",
        "Size (bytes)": len(zstd_comp),
        "Ratio": f"{len(zstd_comp)/orig_size:.2%}",
        "Comp Time (ms)": zstd_time * 1000,
        "Type": "binary"
    })

    # 4. logzip (balanced)
    # Note: we measure the rendered text size as it's what goes to LLM
    start = time.perf_counter()
    res = compress(raw_text, max_ngram=2, max_legend_entries=128)
    rendered = res.render(with_preamble=True)
    logzip_time = time.perf_counter() - start
    results.append({
        "Method": "logzip (balanced)",
        "Size (bytes)": len(rendered.encode("utf-8")),
        "Ratio": f"{len(rendered.encode('utf-8'))/orig_size:.2%}",
        "Comp Time (ms)": logzip_time * 1000,
        "Type": "text/llm"
    })

    # 5. logzip (max)
    start = time.perf_counter()
    res_max = compress(raw_text, max_ngram=2, max_legend_entries=512)
    rendered_max = res_max.render(with_preamble=True)
    logzip_max_time = time.perf_counter() - start
    results.append({
        "Method": "logzip (max)",
        "Size (bytes)": len(rendered_max.encode("utf-8")),
        "Ratio": f"{len(rendered_max.encode('utf-8'))/orig_size:.2%}",
        "Comp Time (ms)": logzip_max_time * 1000,
        "Type": "text/llm"
    })

    print(f"\nBenchmark results for {log_path} ({orig_size/1024/1024:.2f} MB):")
    print("-" * 85)
    print(f"{'Method':<20} | {'Size (KB)':>10} | {'Ratio':>8} | {'Time (ms)':>10} | {'Type':<10}")
    print("-" * 85)
    for r in results:
        size_kb = r['Size (bytes)'] / 1024
        print(f"{r['Method']:<20} | {size_kb:>10.2f} | {r['Ratio']:>8} | {r['Comp Time (ms)']:>10.2f} | {r['Type']:<10}")
    print("-" * 85)
    print("NOTE: logzip produces LLM-readable text, while others produce binary data.")

if __name__ == "__main__":
    benchmark()
