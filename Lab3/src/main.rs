use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};

#[derive(Debug, Serialize, Deserialize)]
struct Task {
    id: u32,
    title: String,
    completed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct TodoList {
    tasks: Vec<Task>,
}

impl TodoList {
    fn new() -> Self {
        TodoList { tasks: Vec::new() }
    }

    fn load_from_file(filename: &str) -> io::Result<Self> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let todo_list = serde_json::from_reader(reader)?;
        Ok(todo_list)
    }

    fn add_task(&mut self, title: String) {
        let id = (self.tasks.len() as u32) + 1;
        let task = Task {
            id,
            title,
            completed: false,
        };
        self.tasks.push(task);
        println!("Завдання додано!");
    }

    fn delete_task(&mut self, id: u32) {
        if let Some(index) = self.tasks.iter().position(|task| task.id == id) {
            self.tasks.remove(index);
            self.reassign_ids();
            println!("Завдання видалено!");
        } else {
            println!("Завдання з таким ID не знайдено.");
        }
    }

    fn reassign_ids(&mut self) {
        for (index, task) in self.tasks.iter_mut().enumerate() {
            task.id = (index as u32) + 1;
        }
    }

    fn edit_task(&mut self, id: u32, new_title: String) {
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) {
            task.title = new_title;
            println!("Завдання оновлено!");
        } else {
            println!("Завдання з таким ID не знайдено.");
        }
    }

    fn mark_completed(&mut self, id: u32) {
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) {
            task.completed = true;
            println!("Завдання позначено як виконане!");
        } else {
            println!("Завдання з таким ID не знайдено.");
        }
    }

    fn list_tasks(&self) {
        if self.tasks.is_empty() {
            println!("Список завдань порожній.");
        } else {
            println!("Список завдань:");
            for task in &self.tasks {
                println!(
                    "{}. {} [{}]",
                    task.id,
                    task.title,
                    if task.completed { "Виконано" } else { "Не виконано" }
                );
            }
        }
    }

    fn save_to_file(&self, filename: &str) -> io::Result<()> {
        let file = File::create(filename)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &self)?;
        println!("Список завдань збережено у файл.");
        Ok(())
    }
}

fn main() {
    let mut todo_list = TodoList::new();
    let filename = "tasks.json";

    if let Ok(loaded_list) = TodoList::load_from_file(filename) {
        todo_list = loaded_list;
    }

    loop {
        todo_list.list_tasks();

        println!("\nМеню:");
        println!("1. Додати завдання");
        println!("2. Видалити завдання");
        println!("3. Редагувати завдання");
        println!("4. Позначити завдання виконаним");
        println!("5. Зберегти та вийти");

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).expect("Помилка введення");
        let choice = choice.trim();

        match choice {
            "1" => {
                println!("Введіть назву завдання:");
                let mut title = String::new();
                io::stdin().read_line(&mut title).expect("Помилка введення");
                todo_list.add_task(title.trim().to_string());
            }
            "2" => {
                todo_list.list_tasks();
                println!("Введіть ID завдання для видалення:");
                let mut id = String::new();
                io::stdin().read_line(&mut id).expect("Помилка введення");
                if let Ok(id) = id.trim().parse::<u32>() {
                    todo_list.delete_task(id);
                } else {
                    println!("Невірний ID.");
                }
            }
            "3" => {
                todo_list.list_tasks();
                println!("Введіть ID завдання для редагування:");
                let mut id = String::new();
                io::stdin().read_line(&mut id).expect("Помилка введення");
                if let Ok(id) = id.trim().parse::<u32>() {
                    println!("Введіть нову назву завдання:");
                    let mut new_title = String::new();
                    io::stdin()
                        .read_line(&mut new_title)
                        .expect("Помилка введення");
                    todo_list.edit_task(id, new_title.trim().to_string());
                } else {
                    println!("Невірний ID.");
                }
            }
            "4" => {
                todo_list.list_tasks();
                println!("Введіть ID завдання для позначення виконаним:");
                let mut id = String::new();
                io::stdin().read_line(&mut id).expect("Помилка введення");
                if let Ok(id) = id.trim().parse::<u32>() {
                    todo_list.mark_completed(id);
                } else {
                    println!("Невірний ID.");
                }
            }
            "5" => {
                if let Err(e) = todo_list.save_to_file(filename) {
                    eprintln!("Помилка збереження: {}", e);
                }
                println!("До побачення!");
                break;
            }
            _ => println!("Невірний вибір. Спробуйте ще раз."),
        }
    }
}
