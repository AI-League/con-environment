from inspect_ai import Task, task
from inspect_ai.dataset import example_dataset, csv_dataset
from inspect_ai.scorer import model_graded_fact
from inspect_ai.solver import (               
  chain_of_thought, generate, self_critique   
)                                             

@task
def theory_of_mind():
    return Task(
        dataset=csv_dataset("./aisi_location_extraction.csv"),
        solver=[
          chain_of_thought(),
          generate(),
          self_critique()
        ],
        scorer=model_graded_fact()
    )