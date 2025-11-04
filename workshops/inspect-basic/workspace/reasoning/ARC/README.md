## ARC (AI2 Reasoning Challenge)

### Dataset

**Source:** [HuggingFace: allenai/ai2_arc](https://huggingface.co/datasets/allenai/ai2_arc)

**Description:** A dataset of 7,787 genuine grade-school level, multiple-choice science questions, assembled to encourage research in advanced question-answering. The dataset is partitioned into two sets: Easy and Challenge. Questions require reasoning, world knowledge, and sometimes complex inference.

**Two Subsets:**

**ARC-Easy:**
- 2,251 questions (test set)
- Questions that are answered correctly by both a retrieval algorithm and a word co-occurrence algorithm
- Easier, more straightforward questions

**ARC-Challenge:**
- 1,172 questions (test set)  
- Questions that are answered incorrectly by both baseline algorithms
- Harder questions requiring more reasoning

**Question Format:**
- Multiple choice (3-5 options)
- Grade-school science topics (physics, biology, chemistry, earth science)
- Answers labeled A, B, C, D, E

**Sample:**
```
Question: "An astronomer observes that a planet rotates faster after a meteorite impact. Which is the most likely effect of this increase in rotation?"

A) Planetary density will decrease.
B) Planetary years will become longer.
C) Planetary days will become shorter.
D) Planetary gravity will become stronger.

Answer: C
```

**Citation:**
```
Clark, P., Cowhey, I., Etzioni, O., Khot, T., Sabharwal, A., Schoenick, C., & Tafjord, O. (2018).
Think you have Solved Question Answering? Try ARC, the AI2 Reasoning Challenge.
arXiv:1803.05457
```

### Running the Evaluation

**Run both subsets:**
```bash
inspect eval ARC/task.py
```

**Run specific subset:**
```bash
# Easy subset only
inspect eval ARC/task.py@arc_easy

# Challenge subset only
inspect eval ARC/task.py@arc_challenge
```

**With specific model:**
```bash
inspect eval ARC/task.py@arc_easy --model openai/gpt-4
inspect eval ARC/task.py@arc_challenge --model anthropic/claude-3-5-sonnet-20241022
```

### Evaluation Logic

**1. Dataset Loading:**

```python
# For ARC-Easy
dataset = hf_dataset(
    path="allenai/ai2_arc",
    name="ARC-Easy",
    split="test",
    sample_fields=record_to_sample,
)

# For ARC-Challenge
dataset = hf_dataset(
    path="allenai/ai2_arc",
    name="ARC-Challenge",
    split="test",
    sample_fields=record_to_sample,
)
```

The `record_to_sample` function:
- Extracts question text
- Extracts choices (labels and text)
- Converts answer key to letter (A, B, C, D, or E)
- Normalizes target to A-E even if original used numbers

**2. Solver Strategy:**

```python
solver = multiple_choice()
```

The `multiple_choice()` solver automatically formats the prompt as:

```
[Question text]

A) [First option]
B) [Second option]
C) [Third option]
D) [Fourth option]
```

No system message or few-shot examples by default.

**3. Scoring:**

```python
scorer = choice()
```

The `choice()` scorer:
- Extracts the letter (A-E) from the model's response
- Handles various formats: "A", "The answer is A", "A)", etc.
- Compares to target letter
- Returns CORRECT or INCORRECT

**4. Metrics:**
- Accuracy: Percentage of questions answered correctly

**Example Flow:**

```
Input to Model:
----
An astronomer observes that a planet rotates faster after a meteorite 
impact. Which is the most likely effect of this increase in rotation?

A) Planetary density will decrease.
B) Planetary years will become longer.
C) Planetary days will become shorter.
D) Planetary gravity will become stronger.
----

Model Output:
----
If a planet rotates faster, it completes one full rotation in less time.
A full rotation defines a day, so the days would become shorter.

The answer is C.
----

Scorer:
Extract: "C"
Target: "C"
Match → CORRECT ✓
```

**Example with Reasoning:**

```
Question: "Which of these would most likely improve the air quality in large Texas cities?"

A) Increasing the number of trash dumps
B) Increasing the number of factories
C) Decreasing the number of automobiles
D) Decreasing the amount of public parks

Model Output:
Air quality in cities is primarily affected by emissions. Automobiles
are a major source of air pollution. Reducing the number of cars would
reduce emissions and improve air quality.

A) Trash dumps can produce methane but aren't the main air quality issue
B) Factories typically increase air pollution
C) This would reduce vehicle emissions - correct
D) Parks help air quality, removing them would worsen it

Answer: C

Scorer: Extract "C" → Target "C" → CORRECT ✓
```

**Key Features:**
- Tests scientific reasoning and world knowledge
- Multiple choice format (easier to grade than open-ended)
- Two difficulty levels (Easy vs Challenge)
- No few-shot by default (but can be added)
- Requires understanding of science concepts
- Some questions need common sense reasoning
- Challenge set filters out questions answerable by simple heuristics

**Expected Performance:**

**ARC-Easy:**
- GPT-4: ~96-98%
- Claude Opus: ~95-97%
- Good models: 85-95%
- Weaker models: 60-80%

**ARC-Challenge:**
- GPT-4: ~88-92%
- Claude Opus: ~85-90%
- Good models: 70-85%
- Weaker models: 40-60%
- Random guessing: ~25% (4 choices typically)

**Why Challenge is Hard:**
- Questions require multi-step reasoning
- Need to combine multiple facts
- May require domain knowledge
- Can't be solved by simple keyword matching
- Often need to eliminate wrong answers through reasoning
