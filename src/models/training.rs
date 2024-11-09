use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct TrainingExercise {
    pub description: String,
    pub success_rate: f64,
}

pub fn get_training_exercises() -> Vec<TrainingExercise> {
    vec![
        TrainingExercise {
            description: "Ты пытаешься поднять ведро воды своим писюном 🪣".to_string(),
            success_rate: 0.6,
        },
        TrainingExercise {
            description: "Ты решил потягать гантели, привязав их к своему писюну 🏋️‍♂️".to_string(),
            success_rate: 0.7,
        },
        TrainingExercise {
            description: "Ты пытаешься открыть бутылку пива своим писюном 🍺".to_string(),
            success_rate: 0.5,
        },
        TrainingExercise {
            description: "Ты решил посетить йогу для писюнов 🧘‍♂️".to_string(),
            success_rate: 0.8,
        },
        TrainingExercise {
            description: "Ты пытаешься набрать текст на клавиатуре своим писюном 💻".to_string(),
            success_rate: 0.4,
        },
    ]
}