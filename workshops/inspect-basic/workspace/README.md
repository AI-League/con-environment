# To do

Put the evals with the data in csv format, with a Inspect task that runs that eval. We also need a markdown file that serves as the "eval card" with the intent of the eval and an engagement scope.

1. Make a folder under `evals` for your evaluation.
2. Add the following files under that folder:   
    - `data.csv`: A CSV file that holds the data that the evaluation will operate on. Use the native [Inspect CSV format](https://inspect.aisi.org.uk/reference/inspect_ai.dataset.html#sample).
    - `task.py`: A Python file with an Inspect Task in it that uses `data.csv`. 
    - `card.md`: A Markdown document describing the **intent** and **scope** of the evaluation.

See the `examples/hellaswag` folder for a full example.

# To run the task
Make sure you have python=3.10+ and [Inspect](https://inspect.aisi.org.uk/) installed.

To run your task: `inspect eval [path-to-task] --model [model-name] --log-dir ./logs/[name-of-your-folder]`
