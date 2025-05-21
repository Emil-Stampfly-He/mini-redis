# Hello Tokio

## 1. Routine
想象你在做一道菜的完整流程——从准备食材、切菜、下锅，到最后装盘。这整个流程就是一个“活”的操作流程。现在，如果你在中间某一步需要等水开，比如“水开了再下锅”，你就会停下来去做别的事，等水开了再回来继续。

在 Rust 的 `async fn` 里，编译器把你的函数“整个做菜流程”打包成一个 **可暂停／可恢复的小任务**（也就是那里的 “routine”）。它内部记录了：

1. 你已经做到了哪一步（比如切好了菜）
2. 还剩下哪些步骤没做（比如等水开、下锅、翻炒）
3. 需要用到的材料（函数里用到的变量）

当你执行到一个 `.await`（“等水开”）时，这个小任务就会**自动存好“书签”**（保存状态），然后让出控制权去干别的事。等等待的事情完成了（“水开”了），运行时再拉你回来，按照书签上的位置继续往下走。

所以 “routine” 就是：

> **编译器根据你的 `async fn` 生成的那个，能够在 `.await` 处暂停和恢复的“带书签”的小任务/小程序”。**

它不是一个真正的操作系统线程，而更像一本你可以在任意位置做书签、随时暂停翻页，然后再从书签处继续读下去的食谱。

## 2. `await`
Rust中的`async fn`本身不会自动跑，而是生成了一个`Future`。这个`Future`就像是一盘还没放进播放器的DVD，里面有完整的电影（也就是函数体），但是还没开始放映。
```rust
async fn say_world() {
    println!("world");
}

#[tokio::main]
async fn main() {
    // Calling `say_world()` does not execute the body of `say_world()`.
    let op = say_world();

    // This println! comes first
    println!("hello");

    // Calling `.await` on `op` starts executing `say_world`.
    op.await;
}
```
在`lep op = say_world()`中，`op`只是拿到了`Future`对象，相当于只拿到了DVD，播放器还没开始启动。

`.await`表达式的意思就是“等到它完成为止”，好比不断去播放这个DVD，直到它播放完毕为止。所以`op.await`就会调用`say_hello`函数体的内容，直到函数体执行完毕。

## 3. `Future`trait
现在关注这个函数：
```rust
async fn say_world() {
    println!("world");
}
```
Rust会在编译时自动生成一个匿名的结构体：
```rust
struct SayWorldFuture {/* state、局部变量...*/}
```
并给他实现`Future<Output = R>`trait（意思是最终会产出一个`R`的`Future`）：
```rust
impl Future for SayWorldFuture {
    type Output = ();
    fn poll(...) -> Poll<String> { /*状态机逻辑*/ }
}
```
这个`SayWorldFuture`就是`Future`的一个特化类型（`Future`接口的实现类），针对`say_world`这个函数体和它的签名专门“定制”出来的，它把函数里所有的局部状态、状态机逻辑都“打包”在了一起。

## 4. `#[tokio::main]`宏
使用`async fn`是因为我们想进入异步上下文。不过，异步函数必须由运行时执行。运行时包含异步任务调度器，提供事件 I/O、计时器等。运行时不会自动启动，因此需要主函数来启动它。
```rust
#[tokio::main]
async fn main() {
    println!("hello");
}
```
实际上是：
```rust
fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("hello");
    })
}
```

# Spawning
## 1. Frame
Redis 的协议叫 RESP（REdis Serialization Protocol），它把一次完整的命令／回复抽象成一个 “frame”——可以是简单字符串（`+OK\r\n`）、错误（`-ERR ...\r\n`）、整数（`:1000\r\n`）、批量字符串（`$6\r\nfoobar\r\n`）或数组（`*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n`）等。每个 frame 就是一整条消息，不是散落在 TCP 流里的若干字节。

`mini-redis`中的`Frame`枚举定义如下：
```rust
#[derive(Clone, Debug)]
pub enum Frame {
    Simple(String),
    Error(String),
    Integer(u64),
    Bulk(Bytes),
    Null,
    Array(Vec<Frame>),
}
```
* **Simple String**：以 `+` 开头的简单字符串
* **Error**：以 `-` 开头的错误消息
* **Integer**：以 `:` 开头的整数
* **Bulk String**：以 `$` 开头的字符串，可能是二进制数据，长度可变
* **Null Bulk String**：特殊的空值，用于表示 NULL
* **Array**：以 `*` 开头，包含多个子帧，支持嵌套

## 2. `tokio::spawn`
### 2.1. Task
这里的`future`其实是一个task，相当于往Tokio线程池中丢一个新任务，这个任务会异步执行，不会阻塞当前线程。

Tokio 任务是一个轻量异步绿色线程。它们通过向 `tokio::spawn` 传递异步块来创建。`tokio::spawn` 函数会返回一个 `JoinHandle`，调用者可以用它与生成的任务交互。异步代码块可能有返回值。调用者可以使用 `JoinHandle` 上的 `.await` 获取返回值。

```rust
let handle: JoinHandle = tokio::spawn(async move { 
    process(socket).await; 
});

let out: Result<()> = handle.await;
```

一个task可以被一个线程执行，也可以被多个线程执行，也可以在多个线程之间移动。上面这个例子就是将task移动（使用了`move`关键字）给一个线程进行执行：
```rust
tokio::spawn(async move { 
    process(socket).await; 
});
```

Task的生命周期是`'static`：
```rust
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    /*...*/
}
```
所以task不能包含对外部数据的引用，因为异步块的生存时间可能长于当前函数。这就是为什么要使用`move`关键字拿到对数据的所有权。
如果希望被`move`的数据能够被获取多次，则应该使用`Arc`指针。

### 2.2. `Send`trait
实现了`Send`trait的类型可以安全地（没有数据竞争和其他未定义行为）按值传给另一个线程。它们可以跨线程移动。

由 `tokio::spawn` 生成的任务必须实现`Send` trait。这允许 Tokio 运行时在线程间移动任务，同时将任务挂起在`.await`中。当task被挂起时，它的`Future`执行权会被让给scheduler。一旦其他线程需要获取task的`Future`执行权，那么该线程就会从scheduler中读取最后的状态数据，并从那时的状态开始执行task。如果task没有实现`Send` trait，那么其他需要执行task的线程没有办法读取task被让出时的状态数据。

## 3. `TcpStream`与入站套接字
这里要区分两种「套接字」（socket）对象的角色：

1. **`TcpListener`** ——用来“监听敲门声”

    * 你在服务端调 `TcpListener::bind(addr).await`，它绑定到某个端口开始监听。
    * 它本身并不能跟客户端读写数据，只能用来接收连接请求。

2. **`TcpStream`** ——代表一条「已建立的连接」

    * 当有客户端真正连进来，调用 `listener.accept().await` 时，Tokio 会为那条连接创建一个新的 `TcpStream`。
    * 这个 `TcpStream` 就是「入站套接字（inbound socket）」，因为它是入站连接（客户端→服务器）那一端的通道。
    * 你接下来要的正是能 `read`/`write`（在 async 里是 `.read().await`/`.write().await`）字节流的对象，这就是 `TcpStream`。

所以：

```rust
let (socket: TcpStream, addr) = listener.accept().await.unwrap();
```

* `listener` 是 `TcpListener`，只管听。
* `accept()` 返回的 `socket` 才是真正跟客户端通讯的句柄，它需要实现异步读写接口（`AsyncRead`/`AsyncWrite`），这正是 `TcpStream` 干的事。

通俗比喻：

> * `TcpListener` 像小区的大门岗亭，负责 “有没有人来敲门？”
> * `TcpStream` 像你给每个进来访客配的房间钥匙，帮你和他们「对话」——听他们说／给他们回话。

因此，「入站套接字」自然就是 `TcpStream` 的类型，因为它就是那条已经建立的、可双向读写的 TCP 连接。

# Shared State
## 1. 线程间共享
为了在多个线程之间共享一个值，通常使用`Arc`和`Mutex`：
1. 使用`Arc`来共享值。
2. 使用`Mutex`来修改值。
3. 使用`Arc<Mutex<T>>`来**共享并修改值**

例如：
```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SharedMap {
    inner: Arc<Mutex<SharedMapInner>>,
}

struct SharedMapInner {
    data: HashMap<i32, String>,
}

impl SharedMap {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SharedMapInner {
                data: HashMap::new(),
            }))
        }
    }

    pub fn insert(&self, key: i32, value: String) {
        let mut lock = self.inner.lock().unwrap();
        lock.data.insert(key, value);
    }

    pub fn get(&self, key: i32) -> Option<String> {
        let lock = self.inner.lock().unwrap();
        lock.data.get(&key).cloned()
    }
}
```
我们的mini-redis数据库就符合“共享且修改”的特征，所以定义如下：
```rust
type Db = Arc<Mutex<HashMap<String, Bytes>>>;
```

## 2. `Deref`trait
看如下代码：
```rust
type Db = Arc<Mutex<HashMap<String, Bytes>>>;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379")
        .await
        .unwrap();

    println!("Listening");

    let db = Arc::new(Mutex::new(HashMap::new()));

    loop {
        // The second item contains the IP and port of the new connection.
        // (TcpStream, SocketAddr)
        let (socket, _) = listener.accept().await.unwrap();

        let db = db.clone();

        println!("Accepted connection");
        // A new task is spawned for each inbound socket. The socket is
        // moved to the new task and processed there.
        tokio::spawn(async move {
            process(socket, db).await;
        });
    }
}

async fn process(socket: TcpStream, db: Db) {
    // The `Connection` lets us read/write redis **frames** instead of
    // byte streams. The `Connection` type is defined by mini-redis.
    let mut connection = Connection::new(socket);
    
    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                let mut db = db.lock().unwrap();
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                let db = db.lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    Frame::Bulk(value.clone().into())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplemented command: {:?}", cmd),
        };

        connection.write_frame(&response).await.unwrap();
    }
}
```
虽然一直在使用指针，但没有任何一个解引用运算符。这是因为这些智能指针都实现了`Deref`trait，实现了自动解引用操作，能够使得我们直接对指向的数据进行操作（但对于某些操作，比如算术操作或者赋值，则需要显示解引用）：
```rust
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    {
        let mut lock: MutexGuard<i32> = mutex.lock().unwrap(); // lock这个变量拿到锁
        *lock += 1; // 算数操作需要显示解引用
    } // lock生命周期结束，锁被释放
   
    do_something_async().await;
}
```

## 3. `Send`trait与死锁
### 3.1. 标准库`Mutex`
假设你有一把锁（mutex），用来保护共享资源。还有两个task都可能去拿这把锁。假设线程没有实现`Send` trait：
   * 任务 A 运行到一半，拿到了这把锁，然后遇到一个 `.await`，于是 Tokio 运行时就把任务 A 暂停，把线程让给别的任务。
   * 这时候，任务 B 被调度到同一条线程上去跑，它也要去拿同一把锁。
   * 任务 B 因为锁被 A 占着，就“卡”住了——而且它是在这条线程上以**同步**方式等待锁的释放，整个线程就被堵住了。
   * 线程被任务 B 的“同步等待”卡住，**根本没法去恢复**被暂停的任务 A。
   * 任务 A 手里又握着锁，却永远拿不到运行机会来把锁释放——这就叫**死锁**。

所以，首先最基本的是必须要保证你的任务是 `Send`（这样 Tokio 能把它扔到其他线程去跑，不会卡在同一条线程上）。但是，即使满足了`Send` trait也不能从根本上避免死锁，可以这样理解：
1. `Send`trait`解决的是“这个值可不可以安全地在线程之间移动”
   * 当一个类型（比如说`MutexGuard`）实现了`Send`trait，编译器就允许你把它跨线程、跨异步任务移动，而不会报错。
   * 但是`Send`trait只是一种**静态**（编译期）的类型约束，它并不会改变Tokio的调度策略，也不会帮你在**运行时**释放锁。
2. 死锁的真正原因是**持锁跨`.await` + 同线程调度**
   * 你在持有锁的同时调用了`.await`，任务挂起。
   * Tokio会把这条任务暂时搁置，让出线程去跑别的任务。
   * 如果下一个被调度到这个线程的任务也去拿同一把锁，就会同步地等在这里，彻底卡死。

最安全的方法是将互斥锁封装在一个结构体中，并且只在该结构体的**非同步方法**中锁定mutex：

```rust
use std::sync::Mutex;

struct CanIncrement {
   mutex: Mutex<i32>,
}

impl CanIncrement {
   fn increment(&self) {
      let mut lock = self.mutex.lock().unwrap();
      *lock += 1;
   }
}

async fn increment_and_do_stuff(can_incr: &CanIncrement) {
   can_incr.increment();
   do_something_async().await;
}
```
这种编程模式能保证不会遇到未实现`Send`trait的错误，因为在异步函数中的任何地方都不会出现`MutexGuard`，而且能够防止死锁。

### 3.2. Tokio`Mutex`
也可以使用Tokio提供的`tokio::sync::Mutex`异步互斥锁。Tokio互斥锁的主要特点是它可以跨`.await`而不会出现任何问题。尽管如此，异步互斥锁比普通互斥锁代价更高，通常最好使用上面的标准库`Mutex`的方法。
```rust
use tokio::sync::Mutex; // note! This uses the Tokio mutex

// This compiles!
// (but restructuring the code would be better in this case)
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock = mutex.lock().await;
    *lock += 1;

    do_something_async().await;
} // lock goes out of scope here
```

## 4. `current_thread` Runtime Flavor
在 Tokio 里，“runtime flavor” 就是指运行时的类型（或调度器类型），它决定了你的异步任务 **怎么被调度执行**。Tokio 提供了两种主要的 flavor：

1. **multithreaded**（也叫 work-stealing 调度器）
2. **`current_thread`**（又称 basic 调度器）

---

### 4.1. `current_thread`是什么

* **单线程运行**
  所有通过它 spawn 的任务，都只会在**同一个**线程上执行，不会跑到其他线程去。
* **轻量级**
  不会为你额外启动若干 worker 线程，只用当前这条线程就能处理事件循环。
* **无需 `Send`**
  既然不跨线程，就不强制要求所有任务都实现 `Send`。
* **没有跨线程竞争**
  互斥锁（`Mutex`）只会在同一条线程里“排队”，不会出现多线程争锁的情况。

### 4.2. 什么时候用`current_thread`

* **任务不多**、只开了一小撮 socket 连接
* 你想给一个同步 API 做“桥接”，把异步调用封装进一个小运行时里
* 不想承担多线程调度的开销／也不需要并行执行

```rust
// 建一个 current_thread 运行时
let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

// 在当前线程上跑一个异步任务
rt.block_on(async {
    // 这里面的所有 .await 都只会在这条线程上执行
});
```

| 特性           | multi-threaded | `current_thread` |
|--------------|----------------|------------------|
| 线程数          | 多个 worker 线程   | 仅当前一条线程          |
| 任务并行度        | 真正并行（多核利用）     | 串行、合作式切换         |
| 强制 `Send` 要求 | 是              | 否                |
| 适合场景         | 高并发、大量任务       | 任务少、轻量级场景        |

**总结**：

* 如果你希望充分利用多核、运行大量异步任务，就用默认的 **multi-threaded**。
* 如果只是要在当前线程上跑一点小玩意儿，不想拆分线程，也不需要 `Send`，就选 **`current_thread`**。

## 5. 细化锁粒度
一开始定义的`Db`是一个整的`HashMap`：
```rust
type Db = Arc<Mutex<HashMap<String, Bytes>>>;
```
锁的粒度太大，性能不好。可以分拆：
```rust
type ShardedDb = Arc<Vec<Mutex<HashMap<String, Vec<u8>>>>>;

fn new_sharded_db(num_shards: usize) -> ShardedDb {
    let mut db = Vec::with_capacity(num_shards);
    for _ in 0..num_shards {
        db.push(Mutex::new(HashMap::new()));
    }
    Arc::new(db)
}
```
或者直接用flurry crate的`ConcurrentHashMap`（按照Java的`ConcurrentHashMap`来实现的线程安全版`HashMap`）：
```rust
type Db = Arc<Mutex<ConcurrentHashMap<String, Bytes>>>;
```

# Channels
## 1. 异步通道
假设客户端需要提交异步地提交两个task：
```rust
#[tokio::main]
async fn main() {
    // Establish a connection to the server
    let mut client = client::connect("127.0.0.1:6379").await.unwrap();

    // Spawn two tasks, one gets a key, the other sets a key
    let t1 = tokio::spawn(async {
        let res = client.get("foo").await;
    });

    let t2 = tokio::spawn(async {
        client.set("foo", "bar".into()).await;
    });

    t1.await.unwrap();
    t2.await.unwrap();
}
```
虽然逻辑上说得通，但是编译显然无法通过：异步块的生命周期可能长于整个函数，所以`t1`在提交的时候就要求必须获取到`client`的所有权。那么`t2`就没有办法获得`client`的所有权，除非使用`Arc<Mutex<T>>`。然而一旦使用了`Mutex`那么`t1`和`t2`的执行就变成同步的了，失去了异步的优势。所以需要换一种方法：Message Passing。

## 2. Message Passing
Message Passing的核心思想是生成（spawn）一个任务，这个任务专门用来再不同任务或线程之间交换信息，而不是直接共享内存。

在`client`中，生成一个叫`manager`的task，`client.`中的所有对外连接和对象全部交由它管理，只有它会去真正执行`get`和`set`等操作。当某个task需要用到`manager`，它并不会自己去抢占锁或者直接调用，而是往一个 **消息通道（channel）** 中发一条请求消息。

## 3. Tokio's Channel
Tokio有几种不同的channel：
* mpsc：多生产者，单消费者
* oneshot：单生产者，单消费者
* broadcast：多生产者，多消费者。可以发送多个值，每个接收者都能看到每个值
* watch：多生产者，多消费者。可以发送多个值，但不保留历史记录。接收者只能看到最近的值

1. 
2. task与`manager`task之间适合使用oneshot channel通信

```rust
// client.rs
#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        value: Bytes,
        resp: Responder<()>,
    },
}

type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;

#[tokio::main]
async fn main() {
    // Capacity at most 32
    let (tx, mut rx) = mpsc::channel(32);
    let tx2 = tx.clone();

    // Manager task
    let manager = tokio::spawn(async move {
        // Establish a connection to the server
        let mut client = client::connect("localhost:6379").await.unwrap();

        // Start receiving messages
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Get { key, resp } => {
                    let result = client.get(&key).await;
                    let _ = resp.send(result);
                }
                Command::Set { key, value, resp} => {
                    let result = client.set(&key, value).await;
                    let _ = resp.send(result);
                }
            }
        }
    });

    let t1 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Get {
            key: "foo".to_string(),
            resp: resp_tx,
        };

        // Send the GET request
        tx.send(cmd).await.unwrap();

        // Await the response
        let resp = resp_rx.await.unwrap();
        println!("GOT = {:?}", resp);
    });

    let t2 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Set {
            key: "foo".to_string(),
            value: "bar".into(),
            resp: resp_tx,
        };

        // Send the SET request
        tx2.send(cmd).await.unwrap();

        // Await the response
        let resp = resp_rx.await.unwrap();
        println!("GOT = {:?}", resp);
    });

    t1.await.unwrap();
    t2.await.unwrap();
    manager.await.unwrap();
}
```