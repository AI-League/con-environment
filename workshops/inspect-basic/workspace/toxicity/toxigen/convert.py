import argparse
import numpy as np
import pandas as pd
import os
import shutil
from pathlib import Path

def write_and_shuffle_prompts(list_of_strings: list, num_sentences):
    """ 
    Given a list of prompt sentences, create an engineered prompt
    Example:
        list_of_strings = ["string1", "string2", "string3"]
        >>> write_and_shuffle_prompts(list_of_strings)
        "- string2\n- string1\n- string3\n-"
    """ 
    if num_sentences > len(list_of_strings):
        num_sentences = len(list_of_strings)
    
    # Remove any trailing newlines from strings
    cleaned_strings = [s.strip() for s in list_of_strings if s.strip()]
    
    if len(cleaned_strings) == 0:
        return ""
    
    selected_indices = np.random.choice(range(len(cleaned_strings)), 
                                      min(num_sentences, len(cleaned_strings)), 
                                      replace=False)
    selected_strings = [cleaned_strings[i] for i in selected_indices]
    
    prompt = ""
    for s in selected_strings:
        prompt += "- {}\\n".format(s.replace("'", ""))
    prompt += "-"
    return prompt

def create_readme(all_sentences_by_sentiment, subject, output_dir):
    """Create a README.md file with all sentences for documentation"""
    readme_path = os.path.join(output_dir, "README.md")
    with open(readme_path, "w") as f:
        f.write(f"# {subject.title()} Sentences\n\n")
        
        total_sentences = sum(len(sentences) for sentences in all_sentences_by_sentiment.values())
        f.write(f"This folder contains {total_sentences} sentences across {len(all_sentences_by_sentiment)} sentiment categories used for prompt generation.\n\n")
        
        for sentiment, sentences in all_sentences_by_sentiment.items():
            f.write(f"## {sentiment.title()} Sentences ({len(sentences)} total):\n\n")
            for i, sentence in enumerate(sentences, 1):
                f.write(f"{i}. {sentence.strip()}\n")
            f.write("\n")

def process_category_folder(folder_path, demonstrations_per_prompt, num_prompts_to_generate):
    """Process a single category folder and return the generated data"""
    folder_name = os.path.basename(folder_path)
    
    # Look for files that match the pattern *_sentences.txt
    sentence_files = []
    for file in os.listdir(folder_path):
        if file.endswith('_sentences.txt'):
            sentence_files.append(file)
    
    if not sentence_files:
        print(f"Warning: No *_sentences.txt files found in {folder_path}")
        return []
    
    # Group files by subject category (e.g., "asian" from "hate_asian_sentences.txt")
    subject_groups = {}
    for sentence_file in sentence_files:
        # Extract subject from filename (e.g., "hate_asian_sentences.txt" -> "asian")
        # Split by underscore, remove "sentences.txt", and take the middle part
        parts = sentence_file.replace('_sentences.txt', '').split('_')
        if len(parts) >= 2:
            subject = '_'.join(parts[1:])  # Everything after the first part (sentiment)
            sentiment = parts[0]  # First part is sentiment (hate/neutral)
            
            if subject not in subject_groups:
                subject_groups[subject] = []
            subject_groups[subject].append((sentence_file, sentiment))
    
    all_data = []
    
    # Process each subject group
    for subject, file_list in subject_groups.items():
        print(f"Processing subject category: {subject}")
        
        # Create output directory for this subject
        output_dir = os.path.join(folder_path, subject)
        os.makedirs(output_dir, exist_ok=True)
        
        subject_data = []
        all_sentences_by_sentiment = {}
        
        # Process each sentiment file for this subject
        for sentence_file, sentiment in file_list:
            file_path = os.path.join(folder_path, sentence_file)
            
            # Read sentences
            with open(file_path, "r", encoding='utf-8') as f:
                sentences = f.readlines()
            
            # Clean sentences
            sentences = [s.strip() for s in sentences if s.strip()]
            
            if not sentences:
                print(f"Warning: No sentences found in {file_path}")
                continue
            
            # Store sentences for README
            all_sentences_by_sentiment[sentiment] = sentences
            
            # Generate prompts for this sentiment
            for i in range(num_prompts_to_generate):
                prompt = write_and_shuffle_prompts(sentences, demonstrations_per_prompt)
                if prompt:  # Only add non-empty prompts
                    subject_data.append({
                        'input': prompt,
                        'target': sentiment
                    })
            
            # Move the original sentence file to the subject folder
            new_file_path = os.path.join(output_dir, sentence_file)
            try:
                shutil.move(file_path, new_file_path)
                print(f"Moved {sentence_file} -> {new_file_path}")
            except Exception as e:
                print(f"Warning: Could not move {sentence_file}: {e}")
        
        # Create comprehensive README for this subject with all sentiments
        if all_sentences_by_sentiment:
            create_readme(all_sentences_by_sentiment, subject, output_dir)
            print(f"Created comprehensive README for subject '{subject}' -> {os.path.join(output_dir, 'README.md')}")
        
        # Save combined CSV for this subject with all sentiments
        if subject_data:
            df = pd.DataFrame(subject_data)
            csv_path = os.path.join(output_dir, "data.csv")
            df.to_csv(csv_path, index=False)
            
            # Count by sentiment
            sentiment_counts = df['target'].value_counts().to_dict()
            print(f"Generated CSV for '{subject}' with {len(subject_data)} total prompts: {sentiment_counts} -> {csv_path}")
        
        all_data.extend(subject_data)
    
    return all_data

def main():
    parser = argparse.ArgumentParser(description="Generate prompts from demonstration sentences and create CSV files")
    parser.add_argument("--demonstrations_folder", type=str, required=True,
                       help="Path to the demonstrations folder containing subfolders with sentence files")
    parser.add_argument("--demonstrations_per_prompt", type=int, default=5,
                       help="Number of sentences to include in each prompt")
    parser.add_argument("--num_prompts_to_generate", type=int, default=100,
                       help="Number of prompts to generate per category")
    parser.add_argument("--output_combined_csv", type=str, default=None,
                       help="Optional: Path to save a combined CSV with all categories")
    
    args = parser.parse_args()
    
    demonstrations_folder = Path(args.demonstrations_folder)
    
    if not demonstrations_folder.exists():
        print(f"Error: Demonstrations folder '{demonstrations_folder}' does not exist")
        return
    
    all_combined_data = []
    
    # Process each subfolder in the demonstrations folder
    for subfolder in demonstrations_folder.iterdir():
        if subfolder.is_dir():
            print(f"\nProcessing subfolder: {subfolder.name}")
            folder_data = process_category_folder(
                str(subfolder), 
                args.demonstrations_per_prompt, 
                args.num_prompts_to_generate
            )
            all_combined_data.extend(folder_data)
    
    # Optionally save combined CSV
    if args.output_combined_csv and all_combined_data:
        combined_df = pd.DataFrame(all_combined_data)
        combined_df.to_csv(args.output_combined_csv, index=False)
        print(f"\nSaved combined CSV with {len(all_combined_data)} total prompts -> {args.output_combined_csv}")
    
    print(f"\nProcessing complete! Generated prompts for {len(set(item['target'] for item in all_combined_data))} categories.")

if __name__ == "__main__":
    main()