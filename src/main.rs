
// Custom Hash Table
// from Crafting Interpreters chapter: https://craftinginterpreters.com/hash-tables.html

use std::rc::Rc;

// Hash table's capacity grows when load factor is at 0.75 not when hash table is full.
const TABLE_MAX_LOAD: f64 = 0.75;

// FNV-1a algorithm: http://www.isthe.com/chongo/tech/comp/fnv/
fn hash_string(key: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for i in 0..key.len() {
        hash ^= key.chars().nth(i).unwrap() as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

fn find_entry<'a, 'b>(entries: &'b mut Vec<Entry<'a>>, capacity: usize, key: &Key) -> &'b mut Entry<'a> {
    let mut index = (key.hash % capacity as u32) as usize;
    let mut tombstone: Option<usize> = None;
    /*
     - If entry.key is None means the entry is not in that bucket. In case of search it tells the entry has not been found
     in case of insert it tells the entry can be inserted there.
     - If entry.key == key means the entry is in that bucket. In case of search the entry has been found and can be returned
     in case of insert, the entry will be overridden by the new one.
     - If the bucket has an entry but with a different key, there is a collision. In that case probing starts: advance until an empty
     bucket is found or a bucket with the same key.
     Infinite loop cannot happen, thanks to load factor at 0.75 it cannot happen that all the buckets are full.
     */
    loop {
        let key_is_none = entries[index].key.is_none();
        let value_is_none = entries[index].value.is_none();
        match (key_is_none, value_is_none) {
            // If we are in an empty bucket and the previous bucket just passed
            // was a Tombstone, that entry is returned so that in case of insert the new Entry
            // goes in the tombstone bucket.
            (true, true) => {
                if let Some(tombstone) = tombstone {
                    return &mut entries[tombstone];
                }
                // understand why is needed
                return &mut entries[index]
            },
            (true, false) => tombstone = Some(index),
            (false, _) => {
                if let Some(key_value) = &entries[index].key {
                    if std::ptr::eq(key.key, key_value.key)  {
                        return &mut entries[index]
                    }
                }
            }
        }
        index += 1;
        index = index % capacity;
    }
}

// The hash value of the key is cached to avoid having to read the whole
// string everytime access is made to the Hash Table.
#[derive(Clone)]
struct Key<'a> {
    key: &'a str,
    hash: u32
}

impl<'a> Key<'a> {
    fn new(key: &'a str) -> Self {
        Key {
            key,
            hash: hash_string(key)
        }
    }
}

#[derive(Clone)]
enum Value {
    Number(f64),
    Boolean(bool),
    String(String),
    Nil
}

impl Value {
    pub fn print_value(&self) {
        match self {
            Value::Number(number) => print!("{}", number),
            Value::Boolean(bool) => print!("{}", bool),
            Value::String(string) => print!("{}", string),
            Value::Nil => print!("nil")
        }
    }
}

#[derive(Clone)]
struct Entry<'a> {
    key: Option<Key<'a>>,
    value: Option<Value>
}

impl<'a> Entry<'a> {
    fn get_key(&'a self) -> &Option<Key<'a>> {
        &self.key
    }

    fn get_value(&self) -> &Option<Value> {
        &self.value
    }
}

struct Table<'a> {
    count: u32,
    capacity: usize,
    entries: Vec<Entry<'a>>
}

impl<'a> Table<'a> {
    fn new(capacity: usize) -> Self {
        Table {
            count: 0,
            capacity,
            entries:  vec![Entry { key: None, value: None }; capacity]
        }
    }

    fn table_set(&mut self, key: Key<'a>, value: Value) -> bool {
        // If number of entries surpass threshold set by load factor, increase the size.
        if (self.count + 1) as f64 > self.capacity as f64 * TABLE_MAX_LOAD {
            let capacity = self.capacity;
            self.adjust_capacity(capacity * 2);
        }
        let capacity = self.capacity;

        let entry = find_entry(&mut self.entries, capacity, &key);
        let is_new_key = entry.get_key().is_none();

        // Increase count only if insert entry in a new bucket, not if replace an already existing key.
        // Is important to also check that the entry.value = None, otherwise it would increase the count also for Tombstone entries.
        // Tombstone entries are considered full buckets in order to account those when adjusting capacity with the load factor, otherwise
        // the risk is having all buckets occupied by Tombstones entries, with no real empty buckets, which would lead to an infinite loop in find_entry().
        if is_new_key && entry.value.is_none() {
            self.count += 1;
        }

        entry.key = Some(key);
        entry.value = Some(value);

        is_new_key
    }

    fn table_get(&mut self, key: &Key) -> Option<Value> {
        if self.count == 0 {
            return None;
        }
        let capacity = self.capacity;
        let entry = find_entry(&mut self.entries, capacity, key);
        if entry.get_key().is_none() {
            None
        } else {
            entry.get_value().clone()
        }
    }

    /*
    Deletion uses the Tombstone approach.
    In Hash Tables implemented using Open Addressing, deletion cannot be done mirroring insertion mechanism: e.g
    walking the buckets until the entry with that key is found and deleting it from the Hash Table.
    In fact, in case of previous collisions, where multiple entries tried to fit in the same bucket, a probe sequence
    has been created.
    Which means that for example:
    - key X goes in bucket 2
    - Key Y collided with X and goes in bucket 3.
    - Key Z collided with X and Y and goes in bucket 4.
    In probe sequences we know that an entry is not present if an empty bucket is hit while walking the sequence.
    If entry Y was deleted by simply setting None bucket 3, then entry Z would become unreachable since every attempt to access
    it would stop at bucket 3.
    The Tombstone approach consists using special entries in place of deleted entries which keep the probe sequence not broken.
     */
    fn table_delete(&mut self, key: &Key) {
        if self.count == 0 {
            return;
        }
        let capacity = self.capacity;
        let entry = find_entry(&mut self.entries, capacity, key);
        if entry.get_key().is_none() {
            return;
        }
        entry.key = None;
        entry.value = Some(Value::Boolean(true));
    }

    fn adjust_capacity(&mut self, capacity: usize) {
        let mut entries: Vec<Entry<'a>> =  vec![Entry { key: None, value: None }; capacity];
        /*for i in 0..capacity {
            entries[i].key = None;
            entries[i].value = None;
        }*/

        // When creating the new array, Tombstones are not re-inserted, therefore the count for the new array
        // won't be the same of the previous array, so we need to clear it out.
        self.count = 0;

        /*
        When choosing the bucket for an entry, the index is computed doing MOD of capacity.
        Therefore, growing the array can move the entries in new buckets.
        To insert each entry in the right bucket the array is rebuilt from scratch copying the old entries
        in the new array.
         */
        for j in 0..self.capacity {
            let entry = &self.entries[j];
            if entry.key.is_none() {
                continue
            }
            if let Some(dest_key) = entry.get_key() {
                let dest = find_entry(&mut entries, capacity, dest_key);
                dest.key = entry.key.clone();
                dest.value = entry.value.clone();
                self.count = self.count + 1;
            }
        }
        self.entries = entries;
        self.capacity = capacity;
    }
}

fn main() {
    let mut hash_table = Table::new(10);
    let key1_str = "key1";
    let key1 = Key::new(key1_str);
    let value1 = Value::String(String::from("value1"));
    hash_table.table_set(key1, value1);
    let key1_test = Key::new(key1_str);
    let maybe_get_value1 = hash_table.table_get(&key1_test);
    if let Some(get_value1) =  maybe_get_value1 {
        get_value1.print_value();
    } else {
        println!("Something is wrong");
    }
}