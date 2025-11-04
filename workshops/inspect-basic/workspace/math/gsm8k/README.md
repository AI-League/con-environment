## GSM8K (Grade School Math 8K)

### Dataset

**Source:** [HuggingFace: gsm8k](https://huggingface.co/datasets/gsm8k)

**Description:** A dataset of 8,500 high-quality, linguistically diverse grade school math word problems created by human problem writers. The problems require multi-step reasoning and basic arithmetic operations (addition, subtraction, multiplication, division).

**Size:** 
- Training: 7,473 problems
- Test: 1,319 problems

**Sample:**
```
Question: "Natalia sold clips to 48 of her friends in April, and then she sold half as many clips in May. How many clips did Natalia sell altogether in April and May?"

Answer: 72

Reasoning: "Natalia sold 48/2 = 24 clips in May. Natalia sold 48+24 = 72 clips altogether in April and May. #### 72"
```

**Citation:**
```
Cobbe, K., Kosaraju, V., Bavarian, M., Chen, M., Jun, H., Kaiser, L., ... & Schulman, J. (2021).
Training Verifiers to Solve Math Word Problems. arXiv:2110.14168
```

### Running the Evaluation

**Basic usage (with 10 few-shot examples, default):**
```bash
inspect eval gsm8k/task.py
```

**With custom few-shot count:**
```bash
# 5 few-shot examples
inspect eval gsm8k/task.py -T fewshot=5

# Zero-shot (no examples)
inspect eval gsm8k/task.py -T fewshot=0
```

**With specific model:**
```bash
inspect eval gsm8k/task.py --model openai/gpt-4
inspect eval gsm8k/task.py --model anthropic/claude-3-5-sonnet-20241022
```

**Custom few-shot seed:**
```bash
inspect eval gsm8k/task.py -T fewshot=10 -T fewshot_seed=123
```

### Evaluation Logic

**1. Dataset Loading:**
```python
dataset = hf_dataset(
    path="gsm8k",
    data_dir="main",
    split="test",
    sample_fields=record_to_sample
)
```

The `record_to_sample` function extracts:
- `input`: The word problem
- `target`: The numerical answer (extracted from after "####")
- `metadata`: The step-by-step reasoning

**2. Solver Strategy:**

The solver uses a prompt template that:
- Presents the math problem
- Instructs the model to think step-by-step
- Requests answer in format "ANSWER: $NUMBER"

**With Few-Shot (default: 10 examples):**
- Loads random training examples
- Shows complete problem + reasoning + answer
- Provides patterns for the model to follow

**Zero-Shot:**
- Just the problem and instructions

**3. Scoring:**

```python
scorer = match(numeric=True)
```

- Extracts answer from model output (looks for "ANSWER: X" pattern)
- Compares numerically to target (so "72" == "72.0")
- Returns CORRECT or INCORRECT

**4. Metrics:**
- Accuracy: Percentage of problems solved correctly

**Example Flow:**

```
Input to Model (with few-shot):
----
[System message with 10 example problems and solutions]

Solve the following math problem step by step...

Natalia sold clips to 48 of her friends in April...

Reasoning:
----

Model Output:
----
Natalia sold 48 clips in April. In May she sold half as many, 
so 48/2 = 24 clips. In total: 48 + 24 = 72 clips.

ANSWER: 72
----

Scorer: Extract "72" → Compare to target "72" → CORRECT ✓
```

**Key Features:**
- Tests multi-step arithmetic reasoning
- Answers are unambiguous (single number)
- Few-shot examples significantly improve performance
- Reasoning quality varies but final answer is what's graded