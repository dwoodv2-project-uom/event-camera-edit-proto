

# Requirements

- Random access read, must be able to read arbitrary events (e.g. event $i$)
	- Manually read entire file and index the files so packets with the same timestamp are grouped up?
		- Probably too slow.
- HDF(4) format is probably the best.
	- HDF4 based file formats:
		- Prophesee Metavision
	- We can use binary search to get the events at any timestamp which is $O(\log(n))$
- AEDAT4 is also possible but it requires the FileDataTable to be present.
	- Also be slower as packets have to be decompressed, (we can probably cache the decompressed packets aswell)
- Try to avoid random access reads as much as possible, try to use sequential reads as much as possible. 


# Solution 1


- Make regular checkpoints every $k$ which store the timestamp of most recent event of pixel $(x,y)$ 
	- Assuming UNIX timestamp 32-bit unsigned:
	- $$\text{Cache size}=\text{width} \times \text{height} \times 4 \ \text{Bytes} $$
	- For DAVIS246 - 346x260 - 359KB per checkpoint
	- Most event camera files are low resolution (?) so this will be relatively low
- Given we want to view time surface a timestamp $x$, where $k_n \leq x \lt k_n+1$ we read & replay the events from $k_n \to x$
	- File read decreased as only events from $k_n \to x$ are read.
	- Can we play from $k_n+1$  if it is closer to $x$? (TODO: do the maths)
- Robust against changes in accumulation type but will use lots of memory usage
- Also investigate making the checkpoints contain more than 1 set of recent timestamps in report (see how much memory usage this uses)
- Also investigate cache eviction strategies, e.g. save checkpoints from $k_n \to k_{n+1}$ with a cache size of 10 for example. if user frequently plays back between these checkpoints -> means less reads. Try different eviction strategies e.g. LRU, FIFO, etc. 

# Solution 2



## Time surface
