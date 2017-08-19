use nl::NeuralLayer;
use sample::Sample;
use matrix::Matrix;
use matrix::MatrixTrait;
use utils::samples_input_to_matrix;
use utils::samples_output_to_matrix;

/// Represents a Neural Network with layers, inputs and outputs
pub struct NeuralNetwork {
    layers: Vec<NeuralLayer>,
    error_fn: Option<Box<Fn(f64)>>,
}

impl NeuralNetwork {
    pub fn new() -> NeuralNetwork {
        NeuralNetwork {
            layers: vec![],
            error_fn: None,
        }
    }

    /// To add a callback function and receive the errors of the network during training process
    /// Please note that there is another function that basically calcualtes the error value
    pub fn error<FN>(&mut self, callback_fn: FN)
    where
        FN: 'static + Fn(f64),
    {
        self.error_fn = Some(Box::new(callback_fn));
    }

    /// To add a new layer to the network
    ///
    /// Example:
    ///
    /// ```
    /// # #[macro_use] extern crate juggernaut;
    /// # fn main() {
    /// use juggernaut::sample::Sample;
    /// use juggernaut::nl::NeuralLayer;
    /// use juggernaut::nn::NeuralNetwork;
    /// use juggernaut::activation::Activation;
    /// use juggernaut::activation::Sigmoid;
    ///
    /// let mut test = NeuralNetwork::new();
    ///
    /// // 1st layer = 4 neurons - 2 inputs
    /// let nl1 = NeuralLayer::new(4, 2, Sigmoid::new());
    ///
    /// test.add_layer(nl1);
    /// # }
    /// ```
    pub fn add_layer(&mut self, layer: NeuralLayer) {
        if self.layers.len() > 0 {
            let prev_layer_neurons = self.layers[self.layers.len() - 1].neurons;

            // 1 for len()
            if prev_layer_neurons != layer.inputs {
                panic!(
                    "New layer should have enough inputs. \
                     Expected {}, got {}",
                    prev_layer_neurons,
                    layer.inputs
                );
            }
        }

        self.layers.push(layer);
    }

    /// This is the forward method of the network which calculates the random weights
    /// and multiplies the inputs of given samples to the weights matrix. Thinks.
    pub fn forward(&self, samples: &Vec<Sample>) -> Vec<Matrix> {
        if self.layers.len() == 0 {
            panic!("Neural network doesn't have any layers.");
        }

        let mut weights: Vec<Matrix> = vec![];

        let mut prev_weight: Matrix = Matrix::zero(0, 0);

        for (i, layer) in self.layers.iter().enumerate() {
            // TODO: this part is ridiculously complicated, needs refactoring.
            // and the reason is Rust's lifetime. clean this part, please.

            if i > 0 {
                let mult: Matrix = prev_weight
                    .dot(&layer.weights)
                    .map(&|n| layer.activation.calc(n));

                if i != self.layers.len() - 1 {
                    prev_weight = mult.clone();
                }

                weights.push(mult);

            } else {
                // first layer (first iteration)
                let samples_input: Matrix = samples_input_to_matrix(&samples);

                let mult: Matrix = samples_input
                    .dot(&layer.weights)
                    .map(&|n| layer.activation.calc(n));

                if self.layers.len() > 1 {
                    // more than one layer
                    // storing the result for the next iteration
                    prev_weight = mult.clone();
                }

                weights.push(mult);
            }
        }

        weights
    }

    /// Use this function to evaluate a trained neural network
    ///
    /// This function simply passes the given sample to the `forward` function and returns the
    /// output of last layer
    pub fn evaluate(&self, sample: Sample) -> Matrix {
        let forward: Vec<Matrix> = self.forward(&vec![sample]);

        // TODO (afshinm): is this correct to clone here?
        forward.last().unwrap().clone()
    }

    /// This function calculates the error of network during training and calls the `error_fn` if
    /// it is available
    ///
    /// This is a private function
    fn _error(&self, error: &Matrix) -> f64 {

        // calculating the median
        let mut error_vec: Vec<f64> = error.row(0).clone();
        error_vec.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = error_vec.len() / 2;

        let err = (if error_vec.len() % 2 == 0 {
            (error_vec[mid - 1] + error_vec[mid + 1]) / 2f64
        } else {
            error_vec[mid] as f64
        }).abs();

        // calling the error_fn
        match self.error_fn {
            Some(ref err_fn) => err_fn(err),
            None => (),
        }

        err
    }

    pub fn train(&mut self, samples: Vec<Sample>, epochs: i32, learning_rate: f64) {
        for _ in 0..epochs {
            let mut output: Vec<Matrix> = self.forward(&samples);

            // because we are backpropagating
            output.reverse();

            //let mut error: Matrix = Matrix::zero(0, 0);
            let mut delta: Matrix = Matrix::zero(0, 0);

            for (i, layer) in output.iter().enumerate() {
                // because it is different when we want to calculate error for each layer for the
                // output layer it is:
                //
                //      y - output_layer
                //
                // but for other layers it is:
                //
                //      output_delta.dot(weights_1)
                //
                let error = if i == 0 {
                    //last layer (output)
                    let samples_outputs: Matrix = samples_output_to_matrix(&samples);

                    // this is:
                    //
                    //     y - last_layer_of_forward
                    //
                    // where `last_layer_of_forward` is `layer` because of i == 0 condition
                    //
                    let error =
                        Matrix::generate(samples_outputs.rows(), samples_outputs.cols(), &|m, n| {
                            samples_outputs.get(m, n) - layer.get(m, n)
                        });

                    // calculating error of this iteration
                    // and call the error_fn to notify
                    self._error(&error);
                    error
                } else {
                    // this is:
                    //
                    //     delta_of_previous_layer.dot(layer)
                    //
                    delta.dot(&self.layers[i].weights.clone().transpose())
                };

                let forward_derivative: Matrix =
                    layer.map(&|n| self.layers[i].activation.derivative(n));
                delta = Matrix::generate(layer.rows(), layer.cols(), &|m, n| {
                    error.get(m, n) * forward_derivative.get(m, n) * learning_rate
                });

                let mut prev_layer: Matrix = samples_input_to_matrix(&samples);

                if i != output.len() - 1 {
                    // TODO (afshinm): is this necessary to clone here?
                    prev_layer = output[output.len() - i - 1].clone();
                }

                // updating weights of this layer
                let syn: Matrix = prev_layer.transpose().dot(&delta);

                let index: usize = self.layers.len() - 1 - i;
                // forward output and network layers are the same, with a reversed order
                // TODO (afshinm): is this necessary to clone here?
                let this_layer_weights: &Matrix = &self.layers[index].weights.clone();

                // finally, set the new weights
                self.layers[index].weights = Matrix::generate(
                    this_layer_weights.rows(),
                    this_layer_weights.cols(),
                    &|m, n| syn.get(m, n) + this_layer_weights.get(m, n),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use activation::Sigmoid;
    use activation::HyperbolicTangent;
    use sample::Sample;
    use nl::NeuralLayer;
    use nn::NeuralNetwork;
    use matrix::MatrixTrait;

    #[test]
    fn forward_test() {
        let dataset = vec![Sample::new(vec![1f64, 0f64], vec![0f64])];

        let mut test = NeuralNetwork::new();

        let sig_activation = Sigmoid::new();
        // 1st layer = 1 neurons - 2 inputs
        test.add_layer(NeuralLayer::new(1, 2, sig_activation));

        let forward = test.forward(&dataset);
        assert_eq!(forward.len(), 1);
    }

    #[test]
    fn forward_test_2layers() {
        let dataset = vec![Sample::new(vec![1f64, 0f64], vec![0f64])];

        let mut test = NeuralNetwork::new();

        let sig_activation = Sigmoid::new();

        // 1st layer = 3 neurons - 2 inputs
        test.add_layer(NeuralLayer::new(3, 2, sig_activation));
        // 2nd layer = 1 neuron - 3 inputs
        test.add_layer(NeuralLayer::new(1, 3, sig_activation));

        let forward = test.forward(&dataset);

        assert_eq!(forward.len(), 2);
    }

    #[test]
    fn train_test() {
        let dataset = vec![Sample::new(vec![1f64, 0f64], vec![0f64])];

        let mut test = NeuralNetwork::new();

        let sig_activation = Sigmoid::new();

        // 1st layer = 1 neurons - 2 inputs
        test.add_layer(NeuralLayer::new(1, 2, sig_activation));

        let forward = test.forward(&dataset);

        test.train(dataset, 10, 0.1f64);

        assert_eq!(forward.len(), 1);
    }

    #[test]
    fn train_test_2layers() {
        let dataset = vec![
            Sample::new(vec![1f64, 0f64], vec![0f64]),
            Sample::new(vec![1f64, 1f64], vec![1f64]),
        ];

        let mut test = NeuralNetwork::new();

        let sig_activation = Sigmoid::new();

        // 1st layer = 3 neurons - 2 inputs
        test.add_layer(NeuralLayer::new(3, 2, sig_activation));
        // 2nd layer = 1 neuron - 3 inputs
        test.add_layer(NeuralLayer::new(1, 3, sig_activation));

        let forward = test.forward(&dataset);

        test.train(dataset, 5, 0.1f64);

        assert_eq!(forward.len(), 2);
    }

    #[test]
    fn train_test_2layers_think() {
        let dataset = vec![
            Sample::new(vec![0f64, 0f64, 1f64], vec![0f64]),
            Sample::new(vec![0f64, 1f64, 1f64], vec![0f64]),
            Sample::new(vec![1f64, 0f64, 1f64], vec![1f64]),
            Sample::new(vec![1f64, 1f64, 1f64], vec![1f64]),
        ];

        let mut test = NeuralNetwork::new();

        let sig_activation = Sigmoid::new();

        // 1st layer = 2 neurons - 3 inputs
        test.add_layer(NeuralLayer::new(2, 3, sig_activation));
        // 2nd layer = 1 neuron - 2 inputs
        test.add_layer(NeuralLayer::new(1, 2, sig_activation));

        test.train(dataset, 5, 0.1f64);

        let think = test.evaluate(Sample::predict(vec![1f64, 0f64, 1f64]));

        assert_eq!(think.rows(), 1);
        assert_eq!(think.cols(), 1);
    }

    #[test]
    fn error_function_test() {
        let dataset = vec![
            Sample::new(vec![0f64, 0f64, 1f64], vec![0f64]),
            Sample::new(vec![0f64, 1f64, 1f64], vec![0f64]),
            Sample::new(vec![1f64, 0f64, 1f64], vec![1f64]),
            Sample::new(vec![1f64, 1f64, 1f64], vec![1f64]),
        ];

        let mut test = NeuralNetwork::new();

        // error should be more than 0
        test.error(|err| assert!(err > 0f64));

        let sig_activation = Sigmoid::new();

        // 1st layer = 2 neurons - 3 inputs
        test.add_layer(NeuralLayer::new(2, 3, sig_activation));
        // 2nd layer = 1 neuron - 2 inputs
        test.add_layer(NeuralLayer::new(1, 2, sig_activation));

        test.train(dataset, 5, 0.1f64);
    }

    #[test]
    fn network_with_two_activations() {
        let dataset = vec![
            Sample::new(vec![0f64, 0f64, 1f64], vec![0f64]),
            Sample::new(vec![0f64, 1f64, 1f64], vec![0f64]),
            Sample::new(vec![1f64, 0f64, 1f64], vec![1f64]),
            Sample::new(vec![1f64, 1f64, 1f64], vec![1f64]),
        ];

        let mut test = NeuralNetwork::new();

        // 1st layer = 2 neurons - 3 inputs
        test.add_layer(NeuralLayer::new(2, 3, Sigmoid::new()));
        // 2nd layer = 1 neuron - 2 inputs
        test.add_layer(NeuralLayer::new(1, 2, HyperbolicTangent::new()));

        test.train(dataset, 5, 0.1f64);

        let think = test.evaluate(Sample::predict(vec![1f64, 0f64, 1f64]));

        assert_eq!(think.rows(), 1);
        assert_eq!(think.cols(), 1);
    }
}
