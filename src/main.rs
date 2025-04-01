use chrono::{DateTime, Days, Months, TimeDelta};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::string::String;
use std::option::Option;

const API_MAX_TIME: Months = Months::new(3);
const NORMAL_WORKING_TIME_PER_DAY_IN_SECONDS: i64 = 7 * 60 * 60;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Shinken Extra Time",
    about = "Calculate extra time worked at Shinken. On the period from J-3months to J-1day"
)]
struct Opt {
    /// Activate debug mode
    #[structopt(short, long)]
    debug: bool,

    /// Generate csv file
    #[structopt(short, long)]
    csv: bool,

    /// Include the actual day in the calculation
    #[structopt(short, long)]
    include_today: bool,

    /// Toggl API Token to use
    #[structopt(short, long)]
    token: Option<String>,
}

struct CSVSheet {
    columns: Vec<Vec<String>>,
    max_columns_length: usize,
    file_name: String,
}

impl CSVSheet {
    fn new(file_name: &str) -> CSVSheet {
        CSVSheet {
            columns: Vec::new(),
            max_columns_length: 0,
            file_name: file_name.to_string(),
        }
    }
    fn add_column(&mut self, column: Vec<String>) {
        self.columns.push(column);
    }

    fn sort_columns(&mut self) {
        self.columns.sort_by(|a, b| a[0].cmp(&b[0]));
    }

    fn update_max_columns_length(&mut self) {
        for column in self.columns.iter_mut() {
            if column.len() > self.max_columns_length {
                self.max_columns_length = column.len();
            }
        }
    }

    fn align_columns(&mut self) {
        self.update_max_columns_length();
        for mut column in self.columns.iter_mut() {
            let len_difference = self.max_columns_length - column.len();
            for _ in 0..len_difference {
                column.push(String::from(""));
            }
        }
    }

    fn add_total_times_to_columns(
        &mut self,
        work_duration_in_seconds_per_day: &HashMap<String, i64>,
        cumulated_extra_time_per_day: &HashMap<String, i64>
    ) {
        self.align_columns();
        for mut column in self.columns.iter_mut() {
            let column_day = column[0].clone();
            let total_work_at_day = work_duration_in_seconds_per_day.get(&column_day).unwrap();
            column.push(String::from(""));
            column.push(String::from("Total time worked that day :"));
            column.push(total_work_at_day.to_string());

            column.push(String::from(""));
            column.push(String::from("Extra time worked that day :"));
            let extra_time_worked_at_day =
                total_work_at_day - NORMAL_WORKING_TIME_PER_DAY_IN_SECONDS;
            column.push(extra_time_worked_at_day.to_string());

            column.push(String::from(""));
            column.push(String::from("Cumulated extra time worked :"));
            column.push(cumulated_extra_time_per_day.get(&column_day).unwrap().to_string())
        }
        self.update_max_columns_length();
    }

    fn write_csv_file(&self) {
        let mut file = File::create(&self.file_name).expect("Could not create CSV file");
        for cell in 0..self.max_columns_length {
            for column in self.columns.iter() {
                write!(&mut file, "{};", column[cell]).expect("Could not write to CSV file");
            }
            write!(&mut file, "\n").expect("Could not write to CSV file");
        }
    }
}

fn main() {
    let opt = Opt::from_args();

    if let Some(token) = opt.token {
        let token = token.as_str();
        let debug = opt.debug;
        let include_today = opt.include_today;
        let mut sheet = CSVSheet::new("results.csv");

        let current_time = chrono::offset::Local::now();
        let query_start = current_time
            .checked_sub_months(API_MAX_TIME)
            .unwrap()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let mut query_end = String::new();
        if include_today {
            query_end = current_time.date_naive().format("%Y-%m-%d").to_string();
        } else {
            query_end = current_time
                .checked_sub_days(Days::new(1))
                .unwrap()
                .date_naive()
                .format("%Y-%m-%d")
                .to_string();
        }

        let mut total_work_duration_per_day: HashMap<String, i64> = HashMap::new();
        let mut all_days = Vec::new();

        println!(
            "Computing extra time worked between {} and {}",
            query_start, query_end
        );

        let url_to_query = format!(
            "https://api.track.toggl.com/api/v9/me/time_entries?start_date={}&end_date={}",
            query_start, query_end
        );

        println!("Querying url: {}", url_to_query);

        let client = reqwest::blocking::Client::new();
        let resp_text = client
            .get(url_to_query)
            .basic_auth(token, Some("api_token"))
            .send()
            .unwrap()
            .text()
            .unwrap();

        let all_tasks: Vec<Value> = serde_json::from_str(&resp_text).unwrap();

        let mut tasks_per_day: HashMap<String, Vec<i64>> = HashMap::new();

        for task in all_tasks.into_iter() {
            let day_as_string = DateTime::parse_from_rfc3339(&task["start"].as_str().unwrap())
                .unwrap()
                .date_naive()
                .format("%Y-%m-%d")
                .to_string();

            let worktime_in_seconds = task["duration"].as_i64().unwrap();

            if tasks_per_day.contains_key(&day_as_string) {
                let mut current_tasks = tasks_per_day.get(&day_as_string).unwrap().clone();
                current_tasks.push(worktime_in_seconds);
                tasks_per_day.remove(&day_as_string);
                tasks_per_day.insert(day_as_string.clone(), current_tasks);
            } else {
                tasks_per_day.insert(
                    day_as_string.clone(),
                    Vec::from([worktime_in_seconds]),
                );
                all_days.push(day_as_string);
            }
        }

        all_days.sort();

        for day in &all_days {
            let day = day.clone();
            let tasks = tasks_per_day.get(&day).unwrap().clone();
            let mut column_to_add_in_sheet = Vec::from([day.clone()]);
            let mut total_worked_that_day = 0;
            for task in tasks.iter() {
                total_worked_that_day += task;
                column_to_add_in_sheet.push(task.to_string());
            }
            sheet.add_column(column_to_add_in_sheet);
            total_work_duration_per_day.insert(day.clone(), total_worked_that_day);
        }

        let mut total_extra_time_worked: i64 = 0;
        let mut cumulated_extra_time_per_day: HashMap<String, i64> = HashMap::new();
        for day in &all_days {
            let time_worked_this_day = total_work_duration_per_day.get(day).unwrap();
            let extra_time_worked = time_worked_this_day - NORMAL_WORKING_TIME_PER_DAY_IN_SECONDS;
            total_extra_time_worked += extra_time_worked;
            cumulated_extra_time_per_day.insert(day.clone(), total_extra_time_worked);
            if debug {
                println!("Extra time worked at day {}: {}", day, extra_time_worked);
            }
        }

        if opt.csv {
            sheet.sort_columns();
            sheet.add_total_times_to_columns(&total_work_duration_per_day, &cumulated_extra_time_per_day);
            sheet.write_csv_file();
        }

        let extra_time_worked: TimeDelta = TimeDelta::seconds(total_extra_time_worked);

        let hours = extra_time_worked.num_hours();
        let minutes = extra_time_worked.num_minutes() - (hours * 60);
        let seconds = extra_time_worked.num_seconds() - (hours * 60 * 60) - (minutes * 60);
        if debug {
            println!(
                "Extra time worked in seconds: {}",
                extra_time_worked.num_seconds()
            );
        }
        println!(
            "Total extra time worked: {}h{}min{}sec",
            hours, minutes, seconds
        );
    } else {
        println!("You need to specify a token");
    }
}
