/*
 * Copyright (c) 2023 Huawei Device Co., Ltd.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Builder to configure the task. Tasks that get spawned through [`TaskBuilder`] inherit all
//! attributes of this builder.
//!
//! A task has following attributes:
//! - priority
//! - task name

use crate::spawn::spawn_async;
use crate::spawn::spawn_blocking;
use crate::task::PriorityLevel;
use crate::JoinHandle;
use std::future::Future;

/// Tasks attribute
#[derive(Clone)]
pub struct TaskBuilder {
    pub(crate) name: Option<String>,
    /// Task priority: ABS_LOW/HIGH/LOW/ABS_LOW
    pub(crate) pri: Option<PriorityLevel>,
    /// Task statistics switch
    pub(crate) is_stat: bool,
    /// Which way of the list the task needs to be inserted
    pub(crate) is_insert_front: bool,
}

impl Default for TaskBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskBuilder {
    /// Creates a new TaskBuilder with a default setting.
    pub fn new() -> Self {
        TaskBuilder {
            name: None,
            pri: None,
            is_stat: false,
            is_insert_front: false,
        }
    }

    /// Sets the name of the task.
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the priority of the task
    pub fn priority(mut self, pri_level: PriorityLevel) -> Self {
        self.pri = Some(pri_level);
        self
    }

    /// Sets whether to turn on task statistics
    pub fn stat(mut self, is_stat: bool) -> Self {
        self.is_stat = is_stat;
        self
    }

    /// Specifies which way of the list to insert the task
    pub fn insert_front(mut self, is_insert_front: bool) -> Self {
        self.is_insert_front = is_insert_front;
        self
    }

    /// todo: for multiple-instance runtime, should provide a spawn_on
    /// Using the current task setting, spawns a task onto the global runtime.
    pub fn spawn<T, R>(&self, task: T) -> JoinHandle<R>
    where
        T: Future<Output = R>,
        T: Send + 'static,
        R: Send + 'static,
    {
        spawn_async(self, task)
    }

    /// Using the current task setting, spawns a task onto the blocking pool.
    pub fn spawn_blocking<T, R>(&self, task: T) -> JoinHandle<R>
    where
        T: FnOnce() -> R,
        T: Send + 'static,
        R: Send + 'static,
    {
        spawn_blocking(self, task)
    }
}

#[cfg(all(test))]
mod test {
    use crate::task::{PriorityLevel, TaskBuilder};

    #[test]
    fn ut_task() {
        ut_builder_new();
        ut_builder_name();
        ut_builder_pri();
        ut_builder_insert_front();
        ut_builder_stat();
    }
    /*
     * @title  Builder::new ut test
     * @design No entry, no exception branch
     * @precon After calling the Builder::new function, get its object
     * @brief  Describe test case execution
     *         1、Checks if the object name property is None
     *         2、Checks if the object pri property is None
     *         3、Checks if the object worker_id property is None
     *         4、Checks if the object is_stat property is false
     *         5、Checks if the object is_insert_front property is false
     * @expect The initialized parameters should correspond to the set ones
     * @auto   true
     */
    fn ut_builder_new() {
        let builder = TaskBuilder::new();
        assert_eq!(builder.name, None);
        assert!(builder.pri.is_none());
        assert!(!builder.is_stat);
        assert!(!builder.is_insert_front);
    }

    /*
     * @title  Builder::name ut test
     * @design No invalid values, no abnormal branches
     * @precon After calling the Builder::new function, get its object
     * @brief  Describe test case execution
     *         1、Checks if the object name property is a modified value
     * @expect Parameters and modified values should correspond to each other
     * @auto   true
     */
    fn ut_builder_name() {
        let builder = TaskBuilder::new();

        let name = String::from("builder_name");
        assert_eq!(builder.name(name.clone()).name.unwrap(), name);
    }

    /*
     * @title  Builder::name ut test
     * @design No invalid values, no abnormal branches
     * @precon After calling the Builder::new function, get its object
     * @brief  Describe test case execution
     *         1、pri set to AbsHigh, check return value
     *         2、pri set to High, check return value
     *         3、pri set to Low, check return value
     *         4、pri set to AbsLow, check return value
     *         5、pri set to Butt, check return value
     * @expect Parameters and modified values should correspond to each other
     * @auto   true
     */
    fn ut_builder_pri() {
        let builder = TaskBuilder::new();
        let pri = PriorityLevel::AbsHigh;
        assert_eq!(builder.priority(pri).pri.unwrap(), pri);

        let builder = TaskBuilder::new();
        let pri = PriorityLevel::High;
        assert_eq!(builder.priority(pri).pri.unwrap(), pri);

        let builder = TaskBuilder::new();
        let pri = PriorityLevel::Low;
        assert_eq!(builder.priority(pri).pri.unwrap(), pri);

        let builder = TaskBuilder::new();
        let pri = PriorityLevel::AbsLow;
        assert_eq!(builder.priority(pri).pri.unwrap(), pri);
    }

    /*
     * @title  Builder::stat ut test
     * @design No invalid values, no abnormal branches
     * @precon After calling the Builder::new function, get its object
     * @brief  Describe test case execution
     *         1、is_stat set to true, check return value
     *         2、is_stat set to false, check return value
     * @expect Parameters and modified values should correspond to each other
     * @auto   true
     */
    fn ut_builder_stat() {
        let builder = TaskBuilder::new();
        let is_stat = true;
        assert_eq!(builder.stat(is_stat).is_stat, is_stat);

        let builder = TaskBuilder::new();
        let is_stat = false;
        assert_eq!(builder.stat(is_stat).is_stat, is_stat);
    }

    /*
     * @title  Builder::insert_front ut test
     * @design No invalid values, no abnormal branches
     * @precon After calling the Builder::new function, get its object
     * @brief  Describe test case execution
     *         1、is_insert_front set to true, check return value
     *         2、is_insert_front set to false, check return value
     * @expect Parameters and modified values should correspond to each other
     * @auto   true
     */
    fn ut_builder_insert_front() {
        let builder = TaskBuilder::new();
        let is_insert_front = true;
        assert_eq!(
            builder.insert_front(is_insert_front).is_insert_front,
            is_insert_front
        );

        let builder = TaskBuilder::new();
        let is_insert_front = false;
        assert_eq!(
            builder.insert_front(is_insert_front).is_insert_front,
            is_insert_front
        );
    }
}
