## AIME 2024 (American Invitational Mathematics Examination)

### Dataset

**Source:** [HuggingFace: Maxwell-Jia/AIME_2024](https://huggingface.co/datasets/Maxwell-Jia/AIME_2024)

**Description:** Problems from the 2024 American Invitational Mathematics Examination (AIME), one of the most prestigious high school mathematics competitions in the United States. AIME problems are significantly harder than AMC problems and are designed for top mathematics students.

**Characteristics:**
- All answers are integers from 0 to 999
- 15 problems per exam (typically 2 exams: AIME I and AIME II)
- Covers algebra, combinatorics, geometry, and number theory
- No multiple choice - pure problem solving
- Extremely challenging (average score ~5-6 out of 15)

**Size:** ~30 problems total (2024 AIME I and II)

**Sample:**
```
Problem: "Let ABCD be a parallelogram with ∠BAD < 90°. A circle tangent to sides DA, AB, and BC intersects diagonal AC at points P and Q with AP < AQ, as shown. Suppose that AP = 3, PQ = 9, and QC = 16. Then the area of ABCD can be written in the form m√n, where m and n are positive integers, and n is not divisible by the square of any prime. Find m + n."

Answer: 650
```

**Source:** Official AIME 2024 competition

### Running the Evaluation

**Basic usage:**
```bash
inspect eval aime/task.py
```

**With specific model:**
```bash
inspect eval aime/task.py --model openai/gpt-4
inspect eval aime/task.py --model anthropic/claude-3-opus-20240229
```

**Note:** This evaluation does not support few-shot examples (dataset is too small to split off training examples).

### Evaluation Logic

**1. Dataset Loading:**
```python
dataset = hf_dataset(
    path="Maxwell-Jia/AIME_2024",
    split="train",
    trust=True,
    sample_fields=record_to_sample,
)
```

The `record_to_sample` function extracts:
- `id`: Problem ID (AIME I #1, AIME II #5, etc.)
- `input`: The problem statement
- `target`: The integer answer (as string)
- `metadata`: The official solution

**2. Solver Strategy:**

Simple zero-shot approach:
```python
solver = [prompt_template(USER_PROMPT_TEMPLATE), generate()]
```

**Prompt Template:**
- Instructs step-by-step solving
- Requests answer in format "ANSWER: $ANSWER"
- Notes no `\boxed{}` command needed
- No system message or few-shot examples

**3. Scoring:**

```python
scorer = match()
```

- Extracts answer from "ANSWER: X" pattern
- Simple string match (no numeric parsing)
- Must match exactly (answers are integers 0-999)

**4. Metrics:**
- Accuracy: Percentage of problems solved correctly

**Example Flow:**

```
Input to Model:
----
Solve the following math problem step by step.
The last line of your response should be of the form "ANSWER: $ANSWER"...

Let ABCD be a parallelogram with ∠BAD < 90°. A circle tangent to 
sides DA, AB, and BC intersects diagonal AC at points P and Q with 
AP < AQ. Suppose that AP = 3, PQ = 9, and QC = 16. Then the area 
of ABCD can be written in the form m√n, where m and n are positive 
integers, and n is not divisible by the square of any prime. 
Find m + n.

Remember to put your answer on its own line...
----

Model Output:
----
Let me work through this step by step.

[Extended reasoning about power of a point, similar triangles, 
parallelogram properties, etc.]

After calculating, the area is 300√13.
So m = 300, n = 13.
Therefore m + n = 313.

Wait, let me recalculate...
[More work]

Actually, the area is 350√13.
m = 350, n = 13
m + n = 363

Hmm, checking again...
[Final calculation]

The area is 325√13, so m = 325, n = 13.

ANSWER: 338

Wait, that should be 325 + 13 = 338. Let me verify...
Actually I made an error. After recalculation: m = 637, n = 13.

ANSWER: 650
----

Scorer: Extract last "ANSWER: 650" → Compare to target "650" → CORRECT ✓
```

**Key Features:**
- Extremely difficult problems (elite competition level)
- All answers are integers 0-999 (easier to grade than MATH dataset)
- Very small dataset (~30 problems total)
- No few-shot (not enough data)
- Zero-shot only - tests raw problem-solving ability
- Expect low accuracy even from best models (5-40% typical)
- Solutions in metadata for analysis

**Expected Performance:**
- GPT-4: ~20-40% accuracy
- Claude Opus: ~15-35% accuracy  
- Smaller models: <10% accuracy
- Human AIME participants: ~33% average (5/15)
- Top human students: 80-100%

**Why This Is Hard:**
1. No multiple choice (can't guess)
2. Multi-step reasoning required
3. Often needs creative insights
4. Competition-level difficulty
5. No examples provided (zero-shot)
6. Must get exact integer answer
