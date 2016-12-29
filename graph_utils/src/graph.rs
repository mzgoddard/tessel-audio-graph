// use std::collections::BTreeMap;

use super::{Node, RingBuffer};

pub struct GraphNodeParams {
    pub to: Vec<usize>,
}

impl Default for GraphNodeParams {
    fn default() -> GraphNodeParams {
        GraphNodeParams {
            to: vec!(),
        }
    }
}

struct GraphNode {
    id: usize,
    params: GraphNodeParams,
    node: Box<Node>,
    input_ids: Vec<(usize, usize)>
    // inputs: Vec<RingBuffer>,
    // outputs: Vec<usize>,
}

impl GraphNode {
    fn new(id: usize, params: GraphNodeParams, node: Box<Node>) -> GraphNode {
        GraphNode {
            id: id,
            params: params,
            node: node,
            input_ids: Vec::new(),
            // inputs: Vec::new(),
            // outputs: params.to.iter().collect(),
        }
    }
}

pub struct Graph {
    next_id: usize,
    nodes: Vec<GraphNode>,
    inputs: Option<Vec<RingBuffer>>,
    outputs: Option<Vec<RingBuffer>>,
    outputs_cache: Option<Vec<Option<Vec<Option<RingBuffer>>>>>,
    // inputs: Vec<usize>,
    // outputs: Vec<usize>,
    // node to list of outputs
    // connections: Map<usize, Vec<usize>>,
    // node to input/buffer pairings
    // buffers: Map<usize, Map<usize, Vec<RingBuffer>>>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            next_id: 0,
            nodes: Vec::new(),
            inputs: Some(Vec::new()),
            outputs: Some(Vec::new()),
            outputs_cache: Some(Vec::new()),
        }
    }

    // fn add_output(&mut self, node: Box<Node>) {}
    pub fn connect(&mut self, node: Box<Node>, params: GraphNodeParams) -> usize {
        let gnode = GraphNode::new(self.next_id, params, node);
        self.next_id += 1;

        let mut outputs = Vec::new();

        for &output_id in gnode.params.to.iter() {
            let output_index = outputs.len();
            outputs.push(Some(RingBuffer::new()));
            self.nodes[output_id].input_ids.push((gnode.id, output_index));
        }

        if let Some(ref mut outputs_cache) = self.outputs_cache {
            outputs_cache.push(Some(outputs));
        }

        self.nodes.push(gnode);

        self.next_id - 1
    }

    // pub fn disconnect(&mut self, node: usize) -> Box<Node> {}
    // fn iter_mut(&mut self) -> Iter<&mut Box<Node>> {}
    // fn iter_inputs(&mut self) -> Iter<&mut Box<Node>> {}
    // fn iter_outputs(&mut self) -> Iter<&mut Box<Node>> {}
    // fn iter_node_inputs(&mut self, node_id: usize) -> Iter <&mut Box<Node>> {}
    // fn wait(&mut self) {}

    pub fn borrow(&self, id: usize) -> &Node {
        &*self.nodes[id].node
    }

    pub fn update(&mut self) {
        let mut inputs = self.inputs.take().unwrap();
        let mut outputs = self.outputs.take().unwrap();
        let mut outputs_cache = self.outputs_cache.take().unwrap();

        for node in self.nodes.iter_mut().rev() {
            // fetch input buffers from output cache
            for &(ref output_id, ref output_index) in node.input_ids.iter() {
                if let Some(ref mut node_outputs) = outputs_cache[*output_id] {
                    inputs.push(node_outputs[*output_index].take().unwrap());
                }
            }

            // update node
            let mut node_outputs = outputs_cache[node.id].take().unwrap();
            for i in 0..node_outputs.len() {
                outputs.push(node_outputs[i].take().unwrap());
            }
            node.node.update(&mut inputs, &mut outputs);
            for i in (0..node_outputs.len()).rev() {
                node_outputs[i] = Some(outputs.pop().unwrap());
            }
            outputs_cache[node.id] = Some(node_outputs);

            // restore input buffers
            for &(ref output_id, ref output_index) in node.input_ids.iter().rev() {
                if let Some(ref mut node_outputs) = outputs_cache[*output_id] {
                    node_outputs[*output_index] = Some(inputs.pop().unwrap());
                }
            }
        }

        self.inputs = Some(inputs);
        self.outputs = Some(outputs);
        self.outputs_cache = Some(outputs_cache);
    }
}

#[cfg(test)]
mod test {
    use super::super::*;
    // use super::*;

    #[test]
    fn it_connects() {
        let mut g = Graph::new();
        let input = Box::new(BaseMix::new());
        let output = Box::new(BaseMix::new());
        let output_id = g.connect(output, GraphNodeParams {
            ..Default::default()
        });
        g.connect(input, GraphNodeParams {
            to: vec!(output_id),
            ..Default::default()
        });
    }

    #[test]
    fn it_updates() {
        let mut g = Graph::new();
        let input = {
            Box::new(Capture::new(Box::new(|output| {
                output.write_from(48, &mut (0i16..48i16).collect());
            })))
        };
        // input.mix_inputs(&mut [RingBuffer::from((0i16..48i16).collect())]);
        let output = Box::new(BaseMix::new());
        let output_id = g.connect(output, GraphNodeParams {
            ..Default::default()
        });
        g.connect(input, GraphNodeParams {
            to: vec!(output_id),
            ..Default::default()
        });
        g.update();
        {
            let output = g.borrow(output_id).downcast_ref::<BaseMix>().unwrap();
            assert_eq!(output.accum.len(), 48);
        }
    }
}
