# Ylong Runtime

## Introduction
Rust language doesn't provide an asynchronous runtime. Instead, it provides basic primitives and functionalities
such as ``async``, ``await``, ``Future``, and ``Waker``. Therefore, it's users' responsibilities to implement the 
runtime, or choose from an existing third party's runtime.

## Introduction of ParIter and its usage

The functions provided by `ylong_runtime::iter` module is similar with other std lambda functions such as `for_each`, but they run in an asynchronous context rather than in a sequential order to speed up. The whole task will be splitted into smaller tasks recursively, and when subtasks complete, their results will be gathered and reduced into a final output.

Current supporting functionality includes:
- for_each
  - If using default settings
    ```rust
    use ylong_runtime::iter::{ParIter, AsParIterMut};
    use ylong_runtime::block_on;
    
    let mut slice = vec![0_u8, 1, 2, 3];
    let handler = slice.par_iter_mut()
                        .for_each(|x| *x += 2);
    block_on(handler).unwrap();
    assert_eq!(
        slice.as_slice(),
        [2, 3, 4, 5]
    );
    ```
    
  - If opening `multi_runtime` feature and using a customized multi-thread runtime
    ```rust
    use ylong_runtime::iter::{ParIter, ParIterBuilder, AsParIterMut};
    use ylong_runtime::builder::RuntimeBuilder;
    use ylong_runtime::block_on;
    
    let mut slice = vec![0_u8, 1, 2, 3];
    
    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let builder = ParIterBuilder::new(&runtime);
    let handler = slice.par_iter_mut_with_builder(&builder)
                        .for_each(|x| *x += 2);
    block_on(handler).unwrap();
    assert_eq!(
        slice.as_slice(),
        [2, 3, 4, 5]
    );
    ```
    
  - If opening `current_thread` feature and using a current thread runtime
    ```rust
    use ylong_runtime::iter::{ParIter, ParIterBuilder, AsParIterMut};
    use ylong_runtime::builder::RuntimeBuilder;
    use ylong_runtime::block_on;
    
    let mut slice = vec![0_u8, 1, 2, 3];
    
    // The runtime passed in can be only set to run with the current thread
    let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
    let builder = ParIterBuilder::new(&runtime);
    let handler = slice.par_iter_mut_with_builder(&builder)
                        .for_each(|x| *x += 2);
    runtime.block_on(handler).unwrap();
    assert_eq!(
        slice.as_slice(),
        [2, 3, 4, 5]
    );
    ```

## Acknowledgements

Based on the user's usage habits, the API of this library retains the original naming style of std after changing the original Rust std synchronous interface implementation to asynchronous, such as ``TcpStream::connect``, ``File::read``, ``File::write``, etc. It also refers to some of Tokio's generic API design ideas, and we would like to express our gratitude to Rust std and Tokio.