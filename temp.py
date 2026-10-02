import json
from pathlib import Path

anime_dir = Path(r"database\bangumi\subject")

result = []

for json_file in anime_dir.glob("*.json"):
    with open(json_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    if isinstance(data, list):
        result.extend(data)
    else:
        result.append(data)

with open("bangumi-subject-combined.json", "w", encoding="utf-8") as f:
    json.dump(result, f, ensure_ascii=False, indent=2)

print(f"合并完成，共 {len(result)} 条记录")