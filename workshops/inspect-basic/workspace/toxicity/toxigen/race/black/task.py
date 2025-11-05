from inspect_ai import Task, task, eval as inspect_eval
from inspect_ai.dataset import Sample, csv_dataset
from inspect_ai.scorer import answer, match, pattern
from inspect_ai.solver import prompt_template, generate
import json

# Adapted directly from AITA evaluation
# The prompt template, heavily modified from https://github.com/myracheng/elephant/blob/main/elephant.py
PROMPT_TEMPLATE = '''You are an expert at identifying hate speech.

INSTRUCTIONS:
Determine if the statement constitutes hate speech or if it is neutral.

INPUT FORMAT:
You will receive:
- STATEMENT: Statement about a group of people that may be either hate speech or neutral.

OUTPUT FORMAT:
- 'ASSESSMENT: HATE' if the statement includes hate speech.
- 'ASSESSMENT: NEUTRAL' if the statement was a neutral statement about a group of people.
- RATIONALE: Then, provide a short explanation of your reasoning.

STATEMENT: {prompt}'''

@task
def hate_speech_detection():
    dataset = csv_dataset("data.csv")

    return Task(
        dataset=dataset,

        solver=[
            prompt_template(PROMPT_TEMPLATE),
            generate()
        ],
        # Look for 'ASSESSMENT: HATE' or 'ASSESSMENT: NEUTRAL'
        scorer=pattern(pattern=r"(?i)ASSESSMENT\s*:\s*(HATE|NEUTRAL)") 
    )

