#!/usr/bin/env python3
"""Download all datasets used in the workshop and save as CSVs."""

import csv
from pathlib import Path
from datasets import load_dataset

# Define all datasets to download
DATASETS = [
    {
        "path": "hellaswag",
        "split": "validation",
        "output": "workspace/reasoning/hellaswag/hellaswag_full.csv",
        "columns": ["ctx", "endings", "label", "source_id"]
    },
    {
        "path": "allenai/ai2_arc",
        "name": "ARC-Easy",
        "split": "test",
        "output": "workspace/reasoning/ARC/arc_easy.csv",
        "columns": ["id", "question", "choices", "answerKey"]
    },
    {
        "path": "allenai/ai2_arc",
        "name": "ARC-Challenge",
        "split": "test",
        "output": "workspace/reasoning/ARC/arc_challenge.csv",
        "columns": ["id", "question", "choices", "answerKey"]
    },
    {
        "path": "EleutherAI/drop",
        "split": "validation",
        "output": "workspace/reasoning/DROP/drop_validation.csv",
        "columns": ["query_id", "passage", "question", "answer", "validated_answers"]
    },
    {
        "path": "hendrycks/competition_math",
        "split": "train",
        "output": "workspace/math/measuring-mathematical-problem-solving/math_train.csv",
        "columns": ["problem", "level", "type", "solution"]
    },
    {
        "path": "Maxwell-Jia/AIME_2024",
        "split": "train",
        "output": "workspace/math/aime/aime_2024.csv",
        "columns": ["ID", "Problem", "Answer", "Solution"]
    },
    {
        "path": "gsm8k",
        "name": "main",
        "split": "test",
        "output": "workspace/math/gsm8k/gsm8k_test.csv",
        "columns": ["question", "answer"]
    },
    {
        "path": "gsm8k",
        "name": "main",
        "split": "train",
        "output": "workspace/math/gsm8k/gsm8k_train.csv",
        "columns": ["question", "answer"]
    },
]

def download_and_save(dataset_config):
    """Download a dataset and save it as CSV."""
    print(f"Downloading {dataset_config['path']} ({dataset_config.get('name', 'default')}) split={dataset_config['split']}...")
    
    # Prepare kwargs for load_dataset
    kwargs = {
        "path": dataset_config["path"],
        "split": dataset_config["split"],
    }
    
    if "name" in dataset_config:
        kwargs["name"] = dataset_config["name"]
    
    # Download dataset using HuggingFace datasets directly
    dataset = load_dataset(**kwargs)
    
    # Create output directory
    output_path = Path(dataset_config["output"])
    output_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Get columns to save
    columns = dataset_config.get("columns", dataset.column_names)
    
    # Convert to CSV
    with open(output_path, 'w', newline='', encoding='utf-8') as f:
        writer = csv.DictWriter(f, fieldnames=columns)
        writer.writeheader()
        
        for item in dataset:
            # Convert complex fields to JSON strings
            row = {}
            for col in columns:
                value = item[col]
                if isinstance(value, (list, dict)):
                    import json
                    row[col] = json.dumps(value)
                else:
                    row[col] = value
            writer.writerow(row)
    
    print(f"✓ Saved to {output_path} ({len(dataset)} rows)")

if __name__ == "__main__":
    for dataset_config in DATASETS:
        try:
            download_and_save(dataset_config)
        except Exception as e:
            print(f"✗ Failed to download {dataset_config['path']}: {e}")
            import traceback
            traceback.print_exc()
    
    print("\nAll datasets downloaded!")
