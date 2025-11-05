## HellaSwag (Commonsense NLI)

### Dataset

**Source:** [HuggingFace: hellaswag](https://huggingface.co/datasets/hellaswag)

**Description:** A challenge dataset for evaluating commonsense natural language inference about physical situations. Questions are sourced from ActivityNet and WikiHow and describe everyday activities. The challenge is to predict how these scenarios end, with adversarially-filtered incorrect endings that are designed to fool models.

**Key Characteristics:**
- Tests commonsense reasoning about physical world
- Scenarios describe everyday activities (cooking, sports, grooming, etc.)
- 4 multiple-choice endings per scenario
- 3 "adversarial" incorrect endings that sound plausible but violate common sense
- Adversarial filtering makes it harder than typical multiple-choice tasks

**Size:**
- Training: 39,905 scenarios
- Validation: 10,042 scenarios
- Test: 10,003 scenarios (labels not public)

**Sample:**
```
Context: "A female chef in white uniform shows a stack of baking pans in a large kitchen presenting them. The pans"

Endings:
A) contain egg yolks and baking soda.
B) are then sprinkled with brown sugar.
C) are placed in a strainer on the counter.
D) are filled with pastries and loaded into the oven.

Answer: D
```

**Citation:**
```
Zellers, R., Holtzman, A., Bisk, Y., Farhadi, A., & Choi, Y. (2019).
HellaSwag: Can a Machine Really Finish Your Sentence?
arXiv:1905.07830
```

### Running the Evaluation

**Basic usage:**
```bash
inspect eval hellaswag/task.py
```

**With specific model:**
```bash
inspect eval hellaswag/task.py --model openai/gpt-4
inspect eval hellaswag/task.py --model anthropic/claude-3-5-sonnet-20241022
```

**Run on your mini CSV version:**
```bash
# Using the small example dataset
cd examples/hellaswag
inspect eval task.py
```

### Evaluation Logic

**1. Dataset Loading:**

**From HuggingFace (full dataset):**
```python
dataset = hf_dataset(
    path="hellaswag",
    split="validation",
    sample_fields=record_to_sample,
    trust=True,
    auto_id=True,
    shuffle=True,
)
```

**From CSV (your example):**
```python
dataset = csv_dataset("data.csv")
# Expects columns: input, choices, target
```

The `record_to_sample` function:
- Extracts context (scenario start)
- Extracts 4 possible endings
- Converts label (0-3) to letter (A-D)
- Stores source_id in metadata

**2. Solver Strategy:**

```python
solver = [
    system_message("Choose the most plausible continuation for the story."),
    multiple_choice()
]
```

**System Message:** Explicitly tells the model the task is choosing plausible continuations

**Multiple Choice Solver:** Formats as:
```
[Context]

A) [First ending]
B) [Second ending]
C) [Third ending]
D) [Fourth ending]
```

**3. Scoring:**

```python
scorer = choice()
```

- Extracts letter (A-D) from model response
- Compares to target letter
- Returns CORRECT or INCORRECT

**4. Metrics:**
- Accuracy: Percentage of scenarios where correct ending was chosen

**Example Flow:**

```
Input to Model:
----
Choose the most plausible continuation for the story.

A female chef in white uniform shows a stack of baking pans in a large 
kitchen presenting them. The pans

A) contain egg yolks and baking soda.
B) are then sprinkled with brown sugar.
C) are placed in a strainer on the counter.
D) are filled with pastries and loaded into the oven.
----

Model Output:
----
The context describes a chef presenting baking pans. The most logical
next step in a baking scenario would be to fill the pans with what 
will be baked (pastries) and then bake them.

A) This describes ingredients, not what happens to the pans
B) Sprinkling pans with sugar doesn't make sense
C) Pans go in the oven, not a strainer
D) This is the natural progression - fill and bake

Answer: D
----

Scorer:
Extract: "D"
Target: "D"  
Match → CORRECT ✓
```

**Example of Adversarial Difficulty:**

```
Context: "Then, the man writes over the snow covering the window of a car, and a woman wearing winter clothes smiles. then"

A) the man adds wax to the windshield and cuts it.
B) a person boards a ski lift, while two men support the head of the person wearing winter clothes as the girls sled.
C) the man puts on a christmas coat, knitted with netting.
D) the man continues removing the snow on his car.

Correct Answer: D

Why the others sound plausible but are wrong:
A) "wax" and "windshield" are related to cars, but cutting wax doesn't make sense
B) Mentions winter activities and people in winter clothes, but random scene change
C) Mentions winter clothing, but doesn't follow from the car scenario
D) Natural continuation - man was clearing snow, continues clearing snow
```

**Key Features:**
- Tests commonsense reasoning about everyday scenarios
- Multiple choice (4 options)
- Adversarial incorrect answers designed to fool models
- Incorrect answers often share keywords/themes with correct answer
- Requires understanding of physical world and typical action sequences
- No complex reasoning needed, just common sense
- Evaluates "situated" language understanding

**Expected Performance:**
- GPT-4: ~95-96%
- Claude Opus: ~93-95%
- Good models: 85-92%
- Weaker models: 60-80%
- Human performance: ~95.6%
- Random guessing: 25% (4 choices)

**What Makes It Challenging:**
1. **Adversarial Filtering:** Wrong answers are chosen to sound plausible
2. **Lexical Overlap:** Wrong answers share words with context
3. **Context Requirement:** Must understand the scenario, not just match keywords
4. **Physical Reasoning:** Need common sense about how the world works
5. **Activity Knowledge:** Need to know typical sequences of actions

**Comparison to Other Tasks:**
- Easier than ARC-Challenge (no domain knowledge needed)
- Easier than MATH/AIME (no calculation)
- Similar difficulty to ARC-Easy for modern LLMs
- But harder than it initially appears due to adversarial design
