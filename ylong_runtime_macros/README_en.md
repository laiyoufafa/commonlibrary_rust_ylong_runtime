## Ylong_macro Process Macros

#### Currently, only the internal implementation macro of the select! macro is contained.

#### Do not use directly

### select! Macro Usage Examples

Basic select with two branches.

```
 async fn do_async1() {
     // do task
 }
 async fn do_async2() {
     // do task
 }
 async fn select_test() {
     ylong_runtime::select! {
         _ = do_async1() => {
             println!("do_async1() completed first");
         },
         _ = do_async2() => {
             println!("do_async2() completed first");
         }
     }
 }
```
Uses if to filter asynchronous tasks
```
 async fn do_async1() -> i32 {
     1
 }
 async fn do_async2() -> i32 {
     2
 }
 async fn do_async3() -> bool {
    false
 }
 async fn select_test() {
     let mut count = 0;
     ylong_runtime::select! {
         a = do_async1(), if false => {
             count += a;
             println!("do_async1() completed first{:?}", a);
         },
         b = do_async2() => {
             count += b;
             println!("do_async2() completed first{:?}", b);
         },
         c = do_async3(), if false => {
             if c {
                 println!("do_async3() completed true");
             }
             else {
                 println!("do_async3() completed false");
             }
         }
     }
     assert_eq!(count, 2);
 }
 ```
Repeated uses select! until all task return.
 ```
 #[cfg(feature = "sync")]
 async fn select_channel() {
     let (tx1, mut rx1) = ylong_runtime::sync::oneshot::channel();
     let (tx2, mut rx2) = ylong_runtime::sync::oneshot::channel();

     ylong_runtime::spawn(async move {
         tx1.send("first").unwrap();
     });

     ylong_runtime::spawn(async move {
         tx2.send("second").unwrap();
     });

     let mut a = None;
     let mut b = None;

     while a.is_none() || b.is_none() {
         ylong_runtime::select! {
             v1 = (&mut rx1), if a.is_none() => a = Some(v1.unwrap()),
             v2 = (&mut rx2), if b.is_none() => b = Some(v2.unwrap()),
         }
     }

     let res = (a.unwrap(), b.unwrap());

     assert_eq!(res.0, "first");
     assert_eq!(res.1, "second");
  }
 ```
Uses 'biased' to execute four task in the specified sequence.
 ```
 async fn select_biased() {
     let mut count = 0u8;

     loop {
         ylong_runtime::select! {
             biased;
             _ = async {}, if count < 1 => {
                 count += 1;
                 assert_eq!(count, 1);
             }
             _ = async {}, if count < 2 => {
                 count += 1;
                 assert_eq!(count, 2);
             }
             _ = async {}, if count < 3 => {
                 count += 1;
                 assert_eq!(count, 3);
             }
             _ = async {}, if count < 4 => {
                 count += 1;
                 assert_eq!(count, 4);
             }
             else => {
                 break;
             }
         }
     }
             
     assert_eq!(count, 4);
  }
 ```
### Subsequent plans to add process macros: Main