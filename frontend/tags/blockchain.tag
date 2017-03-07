<blockchain>
    <table class="table table-striped">
        <thead>
        <tr>
            <th>Date</th>
            <th>Height</th>
        </tr>
        </thead>
        <tbody>
        <tr each={ blocks }>
            <td>{ moment(propose_time * 1000).format('HH:mm:ss, DD MMM YYYY') }</td>
            <td><a href="/#blockchain/block/{ height }">{ height }</a></td>
        </tr>
        </tbody>
    </table>

    <nav>
        <ul class="pager">
            <li class="previous"><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Older</a></li>
            <li class="next"><a href="#" onclick={ next }>Newer <span aria-hidden="true">&rarr;</span></a></a></li>
        </ul>
    </nav>

    <a class="btn btn-lg btn-block btn-default" href="/#">Back</a>

    <script>
        var self = this;

        this.title = 'Blockchain';

        this.api.loadBlockchain(function(data) {
            self.blocks = data;
            self.update();
        });

        previous(e) {
            e.preventDefault();
            self.api.loadBlockchain(self.blocks[0].height - 9, function(data) {
                self.blocks = data;
                self.update();
            });
        }

        next(e) {
            e.preventDefault();
            self.api.loadBlockchain(self.blocks[0].height + 11, function(data) {
                self.blocks = data;
                self.update();
            });
        }
    </script>
</blockchain>