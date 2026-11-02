# Project rules

Copy this file to your project root as `JOTLINE.md` and edit it. jotline sends it to the model with every request, so keep it short.

- Language: Python 3.12. Format with `ruff format`; lint with `ruff check`.
- Verify every change with `ruff check . && pytest -q`.
- Put new modules in `src/<package>/` and their tests in `tests/test_<module>.py`.
- Public functions need type hints and a one-line docstring.
- Use `pathlib.Path`, never `os.path`.
- Do not edit files under `migrations/` or `vendor/`.
