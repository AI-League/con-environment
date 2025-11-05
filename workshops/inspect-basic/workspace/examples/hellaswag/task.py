from inspect_ai import Task, task, eval as inspect_eval
from inspect_ai.dataset import Sample, csv_dataset
from inspect_ai.scorer import choice
from inspect_ai.solver import multiple_choice, system_message
import json

# The context that should be passed along to the model.
SYSTEM_MESSAGE = """
Choose the most plausible continuation for the story.
"""

# The task that constitutes the evaluation.
@task
def hellaswag():
    return Task(
        dataset = csv_dataset("data.csv"),
        solver=[
            system_message(SYSTEM_MESSAGE),
            multiple_choice()
        ],
        scorer=choice()
    )

