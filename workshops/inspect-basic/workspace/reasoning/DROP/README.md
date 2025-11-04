
## DROP (Discrete Reasoning Over Paragraphs)

### Dataset

**Source:** [HuggingFace: EleutherAI/drop](https://huggingface.co/datasets/EleutherAI/drop)

**Description:** A reading comprehension benchmark requiring discrete reasoning over the content of paragraphs. Questions require performing discrete operations like addition, counting, or sorting over entities in the text. Unlike standard reading comprehension, DROP requires multiple reasoning steps and numerical operations.

**Question Types:**
- **Number:** "How many touchdowns were scored in the first quarter?"
- **Date:** "When did the battle occur?"
- **Span:** "Who scored the longest touchdown?" (answer is text span from passage)

**Size:**
- Training: 77,409 questions
- Validation: 9,536 questions

**Sample:**
```
Passage: "In 1763, Spain traded Florida to the Kingdom of Great Britain for control of Havana, Cuba, which had been captured by the British during the Seven Years' War. It was part of a large expansion of British territory following the country's victory in the Seven Years' War..."

Question: "How many years after the British captured Havana did they trade it for Florida?"

Answer: 0 (same year - requires calculating 1763 - 1763)
```

**Citation:**
```
Dua, D., Wang, Y., Dasigi, P., Stanovsky, G., Singh, S., & Gardner, M. (2019).
DROP: A Reading Comprehension Benchmark Requiring Discrete Reasoning Over Paragraphs.
arXiv:1903.00161
```

### Running the Evaluation

**Basic usage (with 3 few-shot examples, default):**
```bash
inspect eval DROP/task.py
```

**With more few-shot examples:**
```bash
inspect eval DROP/task.py -T fewshot=5
```

**Zero-shot (no examples):**
```bash
inspect eval DROP/task.py -T fewshot=0
# or
inspect eval DROP/task.py -T fewshot=false
```

**With specific model:**
```bash
inspect eval DROP/task.py --model openai/gpt-4
inspect eval DROP/task.py --model anthropic/claude-3-5-sonnet-20241022
```

**Custom few-shot seed:**
```bash
inspect eval DROP/task.py -T fewshot=3 -T fewshot_seed=123
```

### Evaluation Logic

**1. Dataset Loading:**
```python
dataset = hf_dataset(
    path="EleutherAI/drop",
    split="validation",
    trust=True,
    sample_fields=record_to_sample,
).filter(_sample_is_not_known_duplicate)
```

The `record_to_sample` function creates:
- `input`: Formatted as "Passage: ...\nQuestion: ...\nAnswer:"
- `target`: List of acceptable answers (multiple valid formats)
- `id`: Query ID
- `metadata`: MD5 hash of passage (for clustering in metrics)

**Note:** Filters out one known duplicate record in the dataset.

**2. Solver Strategy:**

**System Prompt (with few-shot):**
```
You will be asked to read a passage and answer a question. Some examples of passages and Q&A are provided below.

Examples
[3 complete examples with passages, questions, and answers]
```

**User Prompt:**
```
Your Task
---
Passage: [passage text]
Question: [question]
Answer:

Think step by step, then write a line of the form "Answer: $ANSWER" at the end of your response.
```

**3. Scoring:**

```python
scorer = f1(extract_answer)
```

Uses **F1 score** (token-level overlap):
- Precision: % of predicted tokens that appear in target
- Recall: % of target tokens that appear in prediction
- F1 = 2 × (Precision × Recall) / (Precision + Recall)

**Answer Extraction:**
```python
def extract_answer(answer: str) -> str:
    match = re.search(ANSWER_PATTERN, answer)
    return match.group(1) if match else answer
# Looks for "Answer: X" pattern
```

**Multiple Valid Answers:**
The dataset provides multiple acceptable answer formats:
```python
answers = ["5", "five", "5 touchdowns"]
# F1 score is computed against best-matching answer
```

**4. Metrics:**

```python
metrics = [mean(), stderr(cluster="passage_hash")]
```

- **Mean F1**: Average F1 score across all questions
- **Clustered Standard Error**: Accounts for the fact that multiple questions share the same passage (not truly independent samples)

**Example Flow:**

```
Input to Model (with few-shot):
----
[System: Examples of passage + question + answer]

Your Task
---
Passage: In 1763, Spain traded Florida to the Kingdom of Great Britain 
for control of Havana, Cuba, which had been captured by the British 
during the Seven Years' War...

Question: How many years after the British captured Havana did they 
trade it for Florida?

Answer:

Think step by step, then write a line of the form "Answer: $ANSWER"...
----

Model Output:
----
The British captured Havana during the Seven Years' War. The passage 
states that in 1763, Spain traded Florida for Havana. This suggests 
both events happened in 1763, so the answer is 0 years.

Answer: 0
----

Scorer:
Extract: "0"
Target answers: ["0"]
Tokenize: predicted=["0"], target=["0"]
F1 score: 1.0 (perfect match) → CORRECT ✓
----

Alternative Case (Partial Credit):
Model Output: "Answer: zero years"
Tokenize: predicted=["zero", "years"], target=["0"]
Precision: 0/2 = 0 (neither token matches)
Recall: 0/1 = 0
F1: 0.0 → Partial credit possible with synonyms
```

**Example with Spans:**

```
Passage: "...Dwayne Haskins threw a 47-yard touchdown to Terry McLaurin, 
and then a 15-yard touchdown to Steven Sims..."

Question: "Who caught the longest touchdown?"

Target: ["Terry McLaurin"]

Model Output: "Answer: Terry McLaurin caught the longest touchdown pass 
of 47 yards."

Extract: "Terry McLaurin caught the longest touchdown pass of 47 yards"
Tokenize predicted: ["terry", "mclaurin", "caught", "the", "longest", ...]
Tokenize target: ["terry", "mclaurin"]

Precision: 2/11 = 0.18 (only 2 tokens match)
Recall: 2/2 = 1.0 (all target tokens found)
F1: 0.31 (partial credit for extra words)
```

**Key Features:**
- Tests reading comprehension + numerical reasoning
- Requires multi-step reasoning (find info, calculate, format)
- F1 scoring gives partial credit (unlike exact match)
- Multiple valid answer formats accepted
- Clustered standard error accounts for repeated passages
- Few-shot helps model understand expected answer format
- Answers can be numbers, dates, or text spans

**Expected Performance:**
- GPT-4: ~80-85 F1
- Claude Opus: ~75-80 F1
- Strong models: >70 F1
- Weaker models: 40-60 F1
- Human performance: ~96 F1
