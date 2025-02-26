#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug)]
    #[derive(Copy, Clone)]
    struct Node<'a> {
        key: &'a [u8]
    }

    type Nodes<'a> = Vec<&'a Node<'a>>;

    type NodeArray<'a> = &'a [Node<'a>];

    #[test]
    fn test_nodes() {
        let _inode = Node {
            key: &[0, 1],
        };

        let _inode2 = Node {
            key: &[1, 2],
        };

        let mut _nodes = Nodes::new();

        _nodes.push(&_inode);
        _nodes.push(&_inode2);

//        let mut _nodes: Nodes = vec![&_inode, &_inode2];

        let dd: NodeArray = &[
            _inode,
            _inode2,
        ];

        let na = NodeArray::default();

        let i = dd.len();

        _nodes.push(&Node {
            key: &[2, 3],
        });

        println!("inode {:?}", dd);
        println!("inode {:?}", _nodes);

        assert_eq!(_inode.key[0], 0);
        println!("hello world");
    }
}